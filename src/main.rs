pub mod app;
pub mod config;
pub mod editor;
pub mod lsp;
pub mod ui;
pub mod vim;
pub mod git;
pub mod input;
pub mod plugins;

use std::error::Error;
use app::App;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut app = App::new()?;
    let args: Vec<String> = std::env::args().collect();
    app.handle_args(args);
    app.run()?;
    Ok(())
}
