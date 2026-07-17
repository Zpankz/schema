//! `schema` — CLI for the Schema harness control plane.
//!
//! Usage:
//!   schema demo toggle    end-to-end mechanism discovery on ToggleMaze
//!   schema demo cart      representation revision on CartDock
//!   schema selftest       run the harness invariants and both demos
//!   schema help

use schema::agent::SchemaAgent;
use schema::deliberators::{CartDeliberator, ToggleDeliberator};
use schema::envs::{CartDock, ToggleMaze};
use schema::executor::Environment;
use schema::model::WorldModel;

fn print_log(log: &schema::agent::EpisodeLog) {
    for (i, bt) in log.backtests.iter().enumerate() {
        println!("cycle {i}: backtest {bt}");
    }
    for b in &log.bfs_reports {
        println!("{b}");
    }
    for e in &log.executions {
        println!("{e}");
    }
    for n in &log.notes {
        println!("note: {n}");
    }
    println!(
        "won: {} | env actions: {} | model revisions: {}",
        log.won, log.env_actions, log.revisions_seen
    );
}

fn demo_toggle() -> bool {
    println!("=== ToggleMaze: mechanism discovery ===");
    let mut env = ToggleMaze::new();
    let first = env.reset();
    let delib = ToggleDeliberator::new(&first.grid);
    let mut agent = SchemaAgent::new(env, WorldModel::vacuous(), delib);
    let log = agent.run();
    print_log(&log);
    let final_bt = schema::run_backtest(&agent.model, &agent.timeline);
    println!("final certification: {}", final_bt.summary());
    log.won && final_bt.green()
}

fn demo_cart() -> bool {
    println!("=== CartDock: representation revision ===");
    let mut env = CartDock::new();
    let first = env.reset();
    let delib = CartDeliberator::new(&first.grid);
    let mut agent = SchemaAgent::new(env, WorldModel::vacuous(), delib);
    let log = agent.run();
    print_log(&log);
    let final_bt = schema::run_backtest(&agent.model, &agent.timeline);
    println!("final certification: {}", final_bt.summary());
    println!("learned reflow: {:?}", agent.deliberator.reflow);
    log.won && final_bt.green() && agent.deliberator.unified
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let argv: Vec<&str> = args.iter().map(String::as_str).collect();
    let ok = match argv.as_slice() {
        ["demo", "toggle"] => demo_toggle(),
        ["demo", "cart"] => demo_cart(),
        ["selftest"] => {
            let a = demo_toggle();
            println!();
            let b = demo_cart();
            println!();
            println!("selftest: toggle={a} cart={b}");
            a && b
        }
        _ => {
            println!(
                "schema — certified-world-model control loop (jcode-augmented harness)\n\
                 \n\
                   schema demo toggle    mechanism discovery demo\n\
                   schema demo cart      representation-revision demo\n\
                   schema selftest       run both demos and report\n"
            );
            true
        }
    };
    std::process::exit(if ok { 0 } else { 1 });
}
