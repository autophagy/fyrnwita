use serenity::framework::standard::macros::command;
use serenity::framework::standard::{Args, CommandResult};
use serenity::model::channel::Message;
use serenity::prelude::*;

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
