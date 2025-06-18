use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use git2::{BranchType, Repository};
use std::{env, fs, process};

#[derive(Parser, Debug)]
#[command(author)]
struct App {
    #[arg(short, long, default_value = "master")]
    main: String,
    #[command(subcommand)]
    command: Commands,
    #[arg(short, long, default_value_t = true)]
    use_smerge: bool,
}

#[derive(Subcommand, Debug, Clone)]
enum Commands {
    Checkout,
    Checkin,
}

fn main() -> Result<()> {
    let app = App::parse();
    let repo = Repository::discover(".").context("Not a git repository")?;
    let current_branch = repo
        .head()
        .context("Repo may have no commits?")?
        .shorthand()
        .unwrap_or("HEAD")
        .to_string();

    match &app.command {
        Commands::Checkout => {
            // Confirm we're not in a worktree.
            if repo
                .worktrees()?
                .iter()
                .any(|wt| matches!(wt, Some(s) if *s == current_branch))
            {
                bail!("We're in a worktree!");
            }

            let name = format!(
                "scratch-{}",
                petname::petname(2, "-").expect("Wuh? No random?")
            );
            let dest = env::temp_dir().join(&name);

            repo.worktree(&name, &dest, None)?;

            let shell = env::var("SHELL").unwrap_or("/bin/bash".into());
            // We can't affect the shell that spawned us, but we can spawn a subshell!
            process::Command::new(&shell).current_dir(dest).status()?;

            // In theory, we could auto squash merge back in...
            println!(
                "You're back in the real world, the worktree you were in is {}, from {}",
                name, current_branch
            );
        }
        Commands::Checkin => {
            let worktrees = repo.worktrees()?;
            // Confirm we in a worktree.
            if worktrees
                .iter()
                .any(|wt| matches!(wt, Some(s) if *s == current_branch))
            {
                bail!("We're in a worktree!");
            }

            let mut most_recent = None;
            // Find the most recently updated worktree
            for w in worktrees.iter().flatten() {
                let worktree = repo.find_worktree(w)?;
                let path = worktree.path();
                if let Ok(metadata) = fs::metadata(path) {
                    if let Ok(modified_time) = metadata.modified() {
                        match &most_recent {
                            None => {
                                most_recent = Some((worktree, modified_time));
                            }
                            Some((_, current_time)) => {
                                if modified_time > *current_time {
                                    most_recent = Some((worktree, modified_time));
                                }
                            }
                        }
                    }
                }
            }

            let worktree = most_recent.expect("Couldn't find a worktree.").0;
            let dest = worktree.path();
            let w_repo = Repository::open(dest)?;
            let w_head = w_repo.head()?;
            let w_branch = w_head.shorthand().expect("Detached?");

            // Keep it simple, we're just gonna use the command for now

            process::Command::new("git")
                .args(["merge", "--squash", w_branch])
                .status()
                .context("Failed to squash merge")?;

            process::Command::new("git")
                .args(["worktree", "remove", w_branch])
                .status()
                .context("Faliled to remove")?;

            if app.use_smerge {
                process::Command::new("smerge")
                    .arg(".")
                    .status()
                    .context("Unable to open sublime")?;
            } else {
                process::Command::new("git")
                    .arg("commit")
                    .status()
                    .context("Failed to invoke git commit")?;
            }
        }
    }

    Ok(())
}
