use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use serenity::prelude::*;

use std::time::{Duration, Instant};
use time::{format_description, OffsetDateTime};

use crate::SqlitePool;
use sqlx::{Pool, Sqlite};

#[derive(Debug, sqlx::FromRow)]
pub struct Quote {
    id: i32,
    quote: String,
    submitter: String,
    submitted: Option<OffsetDateTime>,
}

#[command]
async fn addquote(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let pool = get_pool(ctx).await;
    let quote = args.rest().trim();
    let submitter = &msg.author.name;
    let submitted = OffsetDateTime::now_utc();

    let query_result = sqlx::query(
        "INSERT INTO quotes (quote, submitter, submitted)
         VALUES(?, ?, ?)",
    )
    .bind(quote)
    .bind(submitter.to_string())
    .bind(submitted)
    .execute(&pool)
    .await;

    let reply = match query_result {
        Ok(result) => {
            format!("Added quote (id: {})", result.last_insert_rowid())
        }
        Err(_) => "Failed to add quote.".to_string(),
    };

    msg.reply(&ctx.http, reply).await?;
    Ok(())
}

#[command]
#[description = "Return a quote from the hord based on the quote's id"]
async fn quoteid(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let start = Instant::now();

    let pool = get_pool(ctx).await;

    let query_result = sqlx::query_as::<_, Quote>(
        "SELECT id, quote, submitter, submitted
         FROM quotes
         WHERE id = ?
         ORDER BY RANDOM() LIMIT 1",
    )
    .bind(args.rest().trim())
    .fetch_one(&pool)
    .await;

    let duration = start.elapsed();
    let reply = quote_message(&query_result, &duration);
    msg.reply(&ctx.http, reply).await?;
    Ok(())
}

#[command]
#[description = "Returns a quote from the hord. No argument returns a random quote."]
async fn quote(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let start = Instant::now();

    let pool = get_pool(ctx).await;

    let query_result = if args.rest().trim() == "" {
        sqlx::query_as::<_, Quote>(
            "SELECT id, quote, submitter, submitted
             FROM quotes
             ORDER BY RANDOM()
             LIMIT 1",
        )
        .fetch_one(&pool)
        .await
    } else {
        sqlx::query_as::<_, Quote>(
            "SELECT quotes.id, highlight(quotes_fts,0,'**','**') quote, quotes.submitter, quotes.submitted
             FROM quotes_fts
             INNER JOIN quotes ON quotes_fts.rowid=quotes.id
             WHERE quotes_fts MATCH ?
             ORDER BY RANDOM() LIMIT 1"
            )
            .bind(args.rest().trim())
            .fetch_one(&pool)
            .await
    };

    let duration = start.elapsed();
    let reply = quote_message(&query_result, &duration);
    msg.reply(&ctx.http, reply).await?;
    Ok(())
}

#[command]
#[owners_only]
#[description = "Expunge a quote from the quote corpus"]
async fn expunge(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let pool = get_pool(ctx).await;

    let query_result = sqlx::query(
        "UPDATE quotes
         SET quote = '< DATA EXPUNGED ON BEHALF OF THE BUREAU OF CHAT HYGIENE >'
         WHERE id = ?",
    )
    .bind(args.rest().trim())
    .execute(&pool)
    .await;

    let reply = match query_result {
        Ok(_) => format!("Expunged quote id: {}", args.rest().trim()),
        Err(_) => "Failed to expunge quote.".to_string(),
    };

    msg.reply(&ctx.http, reply).await?;
    Ok(())
}

fn quote_message(query_result: &Result<Quote, sqlx::Error>, duration: &Duration) -> String {
    match query_result {
        Ok(quote) => {
            if !quote.submitter.is_empty() {
                format!(
                    "[{}] {}\n\n*Submitted by {} on {} [{:.2}ms]*",
                    quote.id,
                    quote.quote,
                    quote.submitter,
                    get_date_with_default(&quote.submitted, "N/A"),
                    duration.as_micros() as f32 / 1000.0,
                )
            } else {
                format!(
                    "[{}] {}\n\n*Submitted on {} [{:.2}ms]*",
                    quote.id,
                    quote.quote,
                    get_date_with_default(&quote.submitted, "N/A"),
                    duration.as_micros() as f32 / 1000.0,
                )
            }
        }
        Err(sqlx::Error::RowNotFound) => "No quote found.".to_string(),
        Err(_) => "Querying error.".to_string(),
    }
}

pub fn get_date_with_default<'a>(value: &'a Option<OffsetDateTime>, default: &'a str) -> String {
    match value {
        Some(d) => {
            let format = format_description::parse(
                "[month repr:short] [day] [year] [hour]:[minute]:[second]",
            )
            .unwrap();
            d.format(&format).unwrap()
        }
        None => default.to_string(),
    }
}

async fn get_pool<'a>(ctx: &Context) -> Pool<Sqlite> {
    let data_read = ctx.data.read().await;
    data_read
        .get::<SqlitePool>()
        .expect("Expected an SqlitePool in TypeMap")
        .clone()
}
