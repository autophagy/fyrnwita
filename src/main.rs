use rand::seq::SliceRandom; // 0.7.2
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
use std::sync::Arc;

use std::time::Instant;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, ReloadPolicy};

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _: Context, ready: Ready) {
        println!("Connected as {}", ready.user.name);
    }
}

struct Queryer {
    parser: QueryParser,
    reader: IndexReader,
    schema: Schema,
}

impl TypeMapKey for Queryer {
    type Value = Arc<Queryer>;
}

#[group]
#[commands(slap, quote)]
struct General;

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
    let index_path = env::var("FW_INDEX").expect("Expected a path to Tantivy index in env");
    let index = Index::open_in_dir(&index_path).unwrap();
    let reader = index
        .reader_builder()
        .reload_policy(ReloadPolicy::OnCommit)
        .try_into()
        .unwrap();
    let mut schema_builder = Schema::builder();

    schema_builder.add_text_field("quote", TEXT | STORED | FAST);
    schema_builder.add_text_field("submitter", TEXT | STORED);
    schema_builder.add_date_field("submitted", STORED);

    let schema = schema_builder.build();

    let parser = QueryParser::for_index(&index, vec![schema.get_field("quote").unwrap()]);

    let queryer = Queryer {
        parser,
        reader,
        schema,
    };

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
        .group(&GENERAL_GROUP);

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&token, intents)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    {
        // Open the data lock in write mode, so keys can be inserted to it.
        let mut data = client.data.write().await;

        data.insert::<Queryer>(Arc::new(queryer));
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

#[command]
async fn quote(ctx: &Context, msg: &Message, args: Args) -> CommandResult {
    let start = Instant::now();

    let queryer = {
        let data_read = ctx.data.read().await;
        data_read
            .get::<Queryer>()
            .expect("Expected queryer")
            .clone()
    };

    let parser = &queryer.parser;
    let reader = &queryer.reader;
    let schema = &queryer.schema;

    let searcher = &reader.searcher();

    let quote = schema.get_field("quote").unwrap();
    let submitter = schema.get_field("submitter").unwrap();

    let query = parser.parse_query(args.rest())?;

    let top_docs = searcher.search(&query, &TopDocs::with_limit(5))?;

    let message = if let Some((score, doc_address)) = top_docs.choose(&mut rand::thread_rng()) {
        let retrieved_doc = searcher.doc(*doc_address)?;
        let quote = get_str_with_default(retrieved_doc.get_first(quote), "");
        let submitter = get_str_with_default(retrieved_doc.get_first(submitter), "");

        let duration = start.elapsed();

        if submitter != "" {
            format!(
                "{}\n\n*Submitted by {} [{:.2} {:.2}ms]*",
                quote,
                submitter,
                score,
                duration.as_micros() as f32 / 1000.0,
            )
        } else {
            format!(
                "{}\n\n*[{:.2} {:.2}ms]*",
                quote,
                score,
                duration.as_micros() as f32 / 1000.0,
            )
        }
    } else {
        "No quote found.".to_string()
    };
    msg.reply(&ctx.http, message).await?;

    Ok(())
}

pub fn get_str_with_default<'a>(value: Option<&'a Value>, default: &'a str) -> &'a str {
    if let Value::Str(i) = value.unwrap() {
        i
    } else {
        default
    }
}
