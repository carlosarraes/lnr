pub mod data;
pub mod render;
pub mod state;
mod terminal;

use crate::{api::ApiClient, auth, error::Result, service::Service};
use crossterm::event::{Event, EventStream};
use futures_util::StreamExt;
use std::sync::Arc;

/// Launch only after the CLI has checked both terminal streams and output mode.
pub async fn run(workspace: Option<&str>) -> Result<()> {
    let service = Arc::new(Service::new(ApiClient::new(
        auth::resolve(workspace).await?,
    )?));
    let mut session = terminal::TerminalSession::enter()?;
    let mut events = EventStream::new();
    let mut loader = data::Loader::new(service);
    let mut app = state::App::new();
    for request in app.start() {
        loader.submit(request);
    }
    loop {
        session
            .terminal
            .draw(|frame| render::render(frame, &mut app))?;
        if app.quit {
            break;
        }
        tokio::select! {
            event = events.next() => match event {
                Some(Ok(Event::Key(key))) => {
                    let previous_screen = app.screen;
                    for request in app.handle_key(key) {
                        loader.submit(request);
                    }
                    if app.screen != previous_screen && !app.loading {
                        loader.cancel_content();
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.into()),
                None => break,
            },
            response = loader.recv() => if let Some(response) = response {
                let issue_loaded = app.accepts(&response)
                    && matches!(&response.result, Ok(state::Payload::Issue(_)));
                app.apply(response);
                if issue_loaded
                    && app.screen == state::Screen::Detail
                    && let Some(request) = app.load_comments()
                {
                    loader.submit(request);
                }
            }
        }
    }
    loader.cancel();
    Ok(())
}
