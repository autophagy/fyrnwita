use std::collections::HashSet;
use std::env;

use serenity::async_trait;
use serenity::framework::standard::macros::{command, group, help, hook};
use serenity::framework::standard::{
    help_commands, Args, CommandGroup, CommandResult, DispatchError, HelpOptions, StandardFramework,
};
use serenity::http::Http;
use serenity::model::channel::Message;
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::UserId;
use serenity::prelude::*;

use std::time::Instant;
use time::{format_description, OffsetDateTime};

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
}

struct SqlitePool;

impl TypeMapKey for SqlitePool {
    type Value = Pool<Sqlite>;
}

#[group]
#[commands(slap)]
struct General;

#[group]
#[commands(quote, quoteid)]
struct Quotes;

#[help]
#[individual_command_tip = "Wes þu hal.\n\n\
For help about a specific command, pass it as an argument to ``!help``.\n"]
#[command_not_found_text = "Could not find: `{}`."]
#[max_levenshtein_distance(3)]
#[strikethrough_commands_tip_in_guild = ""]
async fn help(
    context: &Context,
    msg: &Message,
    args: Args,
    help_options: &'static HelpOptions,
    groups: &[&'static CommandGroup],
    owners: HashSet<UserId>,
) -> CommandResult {
    let _ = help_commands::with_embeds(context, msg, args, help_options, groups, owners).await;
    Ok(())
}

#[hook]
async fn before(_: &Context, msg: &Message, command_name: &str) -> bool {
    println!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );
    true
}

#[hook]
async fn after(_: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{}'", command_name),
        Err(why) => println!("Command '{}' returned error {:?}", command_name, why),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{}'", unknown_command_name);
}

#[hook]
async fn dispatch_error(ctx: &Context, msg: &Message, error: DispatchError, _command_name: &str) {
    if let DispatchError::Ratelimited(info) = error {
        if info.is_first_try {
            let _ = msg
                .channel_id
                .say(
                    &ctx.http,
                    &format!("Try this again in {} seconds.", info.as_secs()),
                )
                .await;
        }
    }
}

#[tokio::main]
async fn main() {
    let db_path = env::var("FW_DB").expect("Expected a path to Sqlite3 DB as $FW_DB");

    let opts = SqliteConnectOptions::new()
        .filename(db_path)
        .journal_mode(SqliteJournalMode::Delete);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    let token = env::var("DISCORD_TOKEN").expect("Expected a token in the environment");

    let http = Http::new(&token);

    let (owners, _) = match http.get_current_application_info().await {
        Ok(info) => {
            let mut owners = HashSet::new();
            if let Some(team) = info.team {
                owners.insert(team.owner_user_id);
            } else {
                owners.insert(info.owner.id);
            }
            match http.get_current_user().await {
                Ok(bot_id) => (owners, bot_id.id),
                Err(why) => panic!("Could not access the bot id: {:?}", why),
            }
        }
        Err(why) => panic!("Could not access application info: {:?}", why),
    };

    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(true).prefix("!").owners(owners))
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .on_dispatch_error(dispatch_error)
        .help(&HELP)
        .group(&GENERAL_GROUP)
        .group(&QUOTES_GROUP);

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<SqlitePool>(pool);
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}

#[command]
#[description = "Attack your target with a 🐟"]
async fn slap(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let target = if args.is_empty() {
        format!("<@{}>", &msg.author.id)
    } else {
        args.rest().to_string()
    };

    msg.channel_id
        .say(
            &ctx.http,
            format!(
                "{} slaps {} around a bit with a large trout",
                &ctx.cache.current_user().name,
                target
            ),
        )
        .await?;
    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct Quote {
    id: i32,
    quote: String,
    submitter: String,
    submitted: Option<OffsetDateTime>,
}

#[command]
#[description = "Return a quote from the hord based on the quote's id"]
async fn quoteid(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let start = Instant::now();

    let pool = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<SqlitePool>()
            .expect("Expected an SqlitePool in TypeMap")
            .clone()
    };

    let stmt =
        "SELECT id, quote, submitter, submitted FROM quotes WHERE id = ? ORDER BY RANDOM() LIMIT 1";
    let query_result = sqlx::query_as::<_, Quote>(stmt)
        .bind(args.rest().trim())
        .fetch_one(&pool)
        .await;

    let duration = start.elapsed();

    match query_result {
        Ok(quote) => {
            let reply = if !quote.submitter.is_empty() {
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
            };

            msg.reply(&ctx.http, reply).await?;
        }
        Err(sqlx::Error::RowNotFound) => {
            msg.reply(&ctx.http, "No quote found.").await?;
        }
        Err(_) => {
            msg.reply(&ctx.http, "Querying error.").await?;
        }
    };

    Ok(())
}

#[command]
#[description = "Returns a quote from the hord. No argument returns a random quote."]
async fn quote(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let start = Instant::now();

    let pool = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<SqlitePool>()
            .expect("Expected an SqlitePool in TypeMap")
            .clone()
    };

    let query_result = if args.rest().trim() == "" {
        let stmt = "SELECT id, quote, submitter, submitted FROM quotes ORDER BY RANDOM() LIMIT 1";
        sqlx::query_as::<_, Quote>(stmt).fetch_one(&pool).await
    } else {
        let stmt = "SELECT quotes.id, highlight(quotes_fts,0,'**','**') quote, quotes.submitter, quotes.submitted FROM quotes_fts INNER JOIN quotes ON quotes_fts.rowid=quotes.id WHERE quotes_fts MATCH ? ORDER BY RANDOM() LIMIT 1";
        sqlx::query_as::<_, Quote>(stmt)
            .bind(args.rest().trim())
            .fetch_one(&pool)
            .await
    };

    let duration = start.elapsed();

    match query_result {
        Ok(quote) => {
            let reply = if !quote.submitter.is_empty() {
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
            };

            msg.reply(&ctx.http, reply).await?;
        }
        Err(sqlx::Error::RowNotFound) => {
            msg.reply(&ctx.http, "No quote found.").await?;
        }
        Err(_) => {
            msg.reply(&ctx.http, "Querying error.").await?;
        }
    };

    Ok(())
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
