mod commands;
mod config;

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use serenity::async_trait;
use serenity::framework::standard::macros::{group, help, hook};
use serenity::framework::standard::{
    help_commands, Args, CommandGroup, CommandResult, DispatchError, HelpOptions, StandardFramework,
};
use serenity::http::Http;
use serenity::model::channel::{Message, ReactionType};
use serenity::model::gateway::{GatewayIntents, Ready};
use serenity::model::id::UserId;
use serenity::prelude::*;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};

use time::OffsetDateTime;

use crate::commands::misc::*;
use crate::commands::quote::*;

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
}

pub struct SqlitePool;

impl TypeMapKey for SqlitePool {
    type Value = Pool<Sqlite>;
}

impl TypeMapKey for config::Configuration {
    type Value = config::Configuration;
}

pub struct CommandCount;

impl TypeMapKey for CommandCount {
    type Value = Arc<AtomicUsize>;
}

pub struct Metadata {
    pub start: OffsetDateTime,
    pub version: String,
}

impl TypeMapKey for Metadata {
    type Value = Metadata;
}

#[group]
#[commands(slap, status)]
struct General;

#[group]
#[commands(addquote, quote, quoteid, expunge)]
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
async fn before(ctx: &Context, msg: &Message, command_name: &str) -> bool {
    println!(
        "Got command '{}' by user '{}'",
        command_name, msg.author.name
    );
    let command_count = {
        let read = ctx.data.read().await;
        read.get::<CommandCount>()
            .expect("Expected CommandCount in TypeMap.")
            .clone()
    };
    command_count.fetch_add(1, Ordering::SeqCst);
    true
}

#[hook]
async fn after(_: &Context, _msg: &Message, command_name: &str, command_result: CommandResult) {
    match command_result {
        Ok(()) => println!("Processed command '{command_name}'"),
        Err(why) => println!("Command '{command_name}' returned error {why:?}"),
    }
}

#[hook]
async fn unknown_command(_ctx: &Context, _msg: &Message, unknown_command_name: &str) {
    println!("Could not find command named '{unknown_command_name}'");
}

#[hook]
async fn normal_message(ctx: &Context, msg: &Message) {
    let data_read = ctx.data.read().await;
    let config = data_read
        .get::<config::Configuration>()
        .expect("Expected a Fyrnwite Configuration in TypeMap");
    let normalized = &msg.content.to_lowercase();

    for (trigger, emoji) in &config.reactions {
        if normalized.contains(trigger) {
            match emoji {
                config::EmojiTypes::Emoji(c) => {
                    msg.react(&ctx, *c).await.unwrap();
                }
                config::EmojiTypes::CustomEmoji(s) => {
                    let reaction = ReactionType::try_from(s.clone()).unwrap();
                    msg.react(&ctx, reaction).await.unwrap();
                }
            };
        }
    }
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
    let start = OffsetDateTime::now_utc();
    let version = env!("CARGO_PKG_VERSION").to_string();
    let discord_token =
        std::env::var("FYRNWITA_DISCORD_TOKEN").expect("Expected FYRNWITA_DISCORD_TOKEN");
    let args: Vec<String> = std::env::args().collect();
    let config_path = &args[1];
    let configuration = config::load_configuration(config_path);

    let hord_path = Path::new(&configuration.hord_path);

    if let Some(p) = hord_path.parent() {
        std::fs::create_dir_all(p).unwrap();
    };

    let opts = SqliteConnectOptions::new()
        .filename(&configuration.hord_path)
        .journal_mode(SqliteJournalMode::Delete)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .unwrap();

    sqlx::migrate!("db/migrations").run(&pool).await.unwrap();

    let http = Http::new(&discord_token);

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
                Err(why) => panic!("Could not access the bot id: {why:?}"),
            }
        }
        Err(why) => panic!("Could not access application info: {why:?}"),
    };

    let framework = StandardFramework::new()
        .configure(|c| c.with_whitespace(true).prefix("!").owners(owners))
        .before(before)
        .after(after)
        .unrecognised_command(unknown_command)
        .normal_message(normal_message)
        .on_dispatch_error(dispatch_error)
        .help(&HELP)
        .group(&GENERAL_GROUP)
        .group(&QUOTES_GROUP);

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&discord_token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    {
        let mut data = client.data.write().await;
        data.insert::<SqlitePool>(pool);
        data.insert::<config::Configuration>(configuration);
        data.insert::<CommandCount>(Arc::new(AtomicUsize::new(0)));
        data.insert::<Metadata>(Metadata { start, version });
    }

    if let Err(why) = client.start().await {
        println!("Client error: {why:?}");
    }
}
