use std::sync::atomic::Ordering;

use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use serenity::prelude::*;

use time::{Duration, OffsetDateTime};

use crate::CommandCount;
use crate::Metadata;
use crate::SqlitePool;

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

#[command]
async fn status(ctx: &Context, msg: &Message, _: Args) -> CommandResult {
    let read = ctx.data.read().await;
    let metadata = read
        .get::<Metadata>()
        .expect("Expected Metadata in TypeMap");
    let mut uptime = OffsetDateTime::now_utc() - metadata.start;
    let uptime_days = uptime.whole_days();
    uptime -= Duration::days(uptime_days);
    let uptime_hours = uptime.whole_hours();
    uptime -= Duration::hours(uptime_hours);
    let uptime_mins = uptime.whole_minutes();
    let command_count = read
        .get::<CommandCount>()
        .expect("Expected a CommandCount in TypeMap")
        .clone()
        .load(Ordering::SeqCst);
    let pool = read
        .get::<SqlitePool>()
        .expect("Expected an SqlitePool in TypeMap")
        .clone();
    let r: (i64,) = sqlx::query_as("SELECT COUNT(*) as quote_count FROM QUOTES")
        .fetch_one(&pool)
        .await?;

    msg.reply(
        &ctx.http,
        format!(
            "
Bot Status:
```
Quotes  :: {}
Queries :: {}
Uptime  :: {}d.{}h.{}m
Version :: {}
```
            ",
            r.0, command_count, uptime_days, uptime_hours, uptime_mins, metadata.version,
        ),
    )
    .await?;
    Ok(())
}
