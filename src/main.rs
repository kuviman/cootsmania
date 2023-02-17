use geng::prelude::*;

#[derive(geng::Assets)]
pub struct Assets {}

pub struct Game {
    geng: Geng,
    assets: Rc<Assets>,
}

impl Game {
    pub fn new(geng: &Geng, assets: &Rc<Assets>) -> Self {
        Self {
            geng: geng.clone(),
            assets: assets.clone(),
        }
    }
}

impl geng::State for Game {
    fn draw(&mut self, framebuffer: &mut ugli::Framebuffer) {
        ugli::clear(framebuffer, Some(Rgba::BLACK), None, None);
    }
}

#[derive(clap::Parser)]
struct Args {
    #[clap(flatten)]
    geng: geng::CliArgs,
}

fn main() {
    logger::init().unwrap();
    geng::setup_panic_handler();
    let args: Args = clap::Parser::parse();
    let geng = Geng::new_with(geng::ContextOptions {
        title: "Coots".to_owned(),
        ..geng::ContextOptions::from_args(&args.geng)
    });
    geng::run(
        &geng,
        geng::LoadingScreen::new(&geng, geng::EmptyLoadingScreen, {
            let geng = geng.clone();
            async move {
                let assets: Assets = geng
                    .load_asset(run_dir().join("assets"))
                    .await
                    .expect("Failed to load assets");
                let assets = Rc::new(assets);
                Game::new(&geng, &assets)
            }
        }),
    );
}
