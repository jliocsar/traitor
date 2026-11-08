use clap::Parser;

#[derive(Parser)]
#[command(name = "traitor", version, about)]
struct Arguments {}

fn main() {
    Arguments::parse();
}
