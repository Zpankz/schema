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
        ["demo", "protocol"] => demo_protocol(),
        ["selftest"] => {
            let a = demo_toggle();
            println!();
            let b = demo_cart();
            println!();
            let c = demo_protocol();
            println!();
            println!("selftest: toggle={a} cart={b} protocol={c}");
            a && b && c
        }
        _ => {
            println!(
                "schema — certified-world-model control loop (jcode-augmented harness)\n\
                 \n\
                   schema demo toggle      mechanism discovery (native deliberator)\n\
                   schema demo cart        representation-revision demo\n\
                   schema demo protocol    discovery driven ENTIRELY via the JSON\n\
                   \x20                     deliberation protocol — the world program is\n\
                   \x20                     authored as text by a provider-shaped function\n\
                   schema selftest         run all demos and report\n"
            );
            true
        }
    };
    std::process::exit(if ok { 0 } else { 1 });
}

/// The world program is authored entirely via the JSON deliberation protocol
/// by a provider-shaped `&str -> String` responder — the working jcode seam.
fn demo_protocol() -> bool {
    use schema::demo_provider::toggle_provider;
    use schema::program::RuleProgram;
    use schema::protocol::ProtocolDeliberator;

    println!("=== ToggleMaze via the JSON deliberation protocol ===");
    let mut env = ToggleMaze::new();
    env.reset();
    let delib = ProtocolDeliberator::new(toggle_provider);
    let model = WorldModel::from_program(RuleProgram::vacuous(), "vacuous program");
    let mut agent = SchemaAgent::new(env, model, delib);
    let log = agent.run();
    print_log(&log);
    let final_bt = schema::run_backtest(&agent.model, &agent.timeline);
    println!("final certification: {}", final_bt.summary());
    if let Some(p) = agent.model.program() {
        println!("world program authored via JSON (readable as text):\n{}", p.to_pretty());
    }
    println!("JSON deliberation turns: {}", agent.deliberator.transcript.len());
    log.won && final_bt.green()
}
