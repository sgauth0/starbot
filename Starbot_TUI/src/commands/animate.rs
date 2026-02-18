use std::io::{self, stdout};
use std::thread;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use clap::Parser;

use crate::app::Runtime;
use crate::errors::CliError;

#[derive(Debug, Clone, Parser)]
pub struct AnimateArgs;

pub async fn handle(_runtime: &Runtime, _args: AnimateArgs) -> Result<(), CliError> {
    let mut terminal = setup_terminal()?;
    let result = run_animation_builder(&mut terminal);
    teardown_terminal(&mut terminal)?;
    result
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>, CliError> {
    let stdout = stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend).map_err(|e| CliError::Generic(e.to_string()))?;

    enable_raw_mode().map_err(|e| CliError::Generic(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        EnterAlternateScreen,
    )
    .map_err(|e| CliError::Generic(e.to_string()))?;

    Ok(terminal)
}

fn teardown_terminal(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), CliError> {
    disable_raw_mode().map_err(|e| CliError::Generic(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
    )
    .map_err(|e| CliError::Generic(e.to_string()))?;
    terminal.show_cursor().map_err(|e| CliError::Generic(e.to_string()))?;
    Ok(())
}

struct AnimationBuilder {
    frames: Vec<String>,
    current_frame: usize,
    name: String,
    speed: u64,
}

impl AnimationBuilder {
    fn new(name: &str, frames: Vec<&str>, speed_ms: u64) -> Self {
        Self {
            frames: frames.into_iter().map(|s| s.to_string()).collect(),
            current_frame: 0,
            name: name.to_string(),
            speed: speed_ms,
        }
    }

    fn from_strings(name: &str, frames: Vec<String>, speed_ms: u64) -> Self {
        Self {
            frames,
            current_frame: 0,
            name: name.to_string(),
            speed: speed_ms,
        }
    }

    fn next_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % self.frames.len();
    }

    fn current(&self) -> &str {
        &self.frames[self.current_frame]
    }
}

fn run_animation_builder(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
) -> Result<(), CliError> {
    let mut animations: Vec<AnimationBuilder> = vec![
        AnimationBuilder::new("Block (3-frame)", vec!["▓", "▒", "░"], 200),
        AnimationBuilder::new("Block (4-frame pulse)", vec!["▓", "▒", "░", "▒"], 200),
        AnimationBuilder::new("Classic spinner", vec!["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"], 80),
        AnimationBuilder::new("Dots", vec!["⠋", "⠙", "⠚", "⠞", "⠖", "⠦", "⠴", "⠲", "⠳", "⠓"], 80),
        AnimationBuilder::new("Arrow", vec!["←", "↖", "↑", "↗", "→", "↘", "↓", "↙"], 100),
        AnimationBuilder::new("Box", vec!["◰", "◳", "◲", "◱"], 150),
        AnimationBuilder::new("Pulse", vec!["◯", "◉"], 250),
        AnimationBuilder::new("Slash", vec!["/", "-", "\\", "|"], 150),
    ];

    let mut selected_idx = 0;
    let mut playing_idx: Option<usize> = None;
    let mut frame_count = 0;
    let mut mode = AppMode::Browse;

    loop {
        // Draw UI
        terminal
            .draw(|f| {
                let size = f.area();
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Length(3),
                        Constraint::Min(10),
                        Constraint::Length(4),
                    ])
                    .split(size);

                // Title
                let title = match mode {
                    AppMode::Browse => "Animation Builder - ↑/↓ select, ENTER play, A add new, Q quit",
                    AppMode::Creating(_) => "Add New Animation - Paste frames with quotes (e.g., \"✦\" on each line, then empty line to finish)",
                };
                let title = Paragraph::new(title)
                    .style(Style::default().fg(Color::Cyan))
                    .alignment(Alignment::Center);
                f.render_widget(title, chunks[0]);

                // List of animations
                let mut list_content = String::new();
                for (i, anim) in animations.iter().enumerate() {
                    let marker = if i == selected_idx { "> " } else { "  " };
                    let playing = if Some(i) == playing_idx {
                        format!(" [{}]", anim.current())
                    } else {
                        String::new()
                    };
                    list_content.push_str(&format!(
                        "{}{}: {} ({}ms){}\n",
                        marker, i + 1, anim.name, anim.speed, playing
                    ));
                }

                // Add "new animation" option
                let marker = if selected_idx == animations.len() { "> " } else { "  " };
                list_content.push_str(&format!("{}{}. [+ Add New Animation]\n", marker, animations.len() + 1));

                let list = Paragraph::new(list_content)
                    .block(Block::default().borders(Borders::ALL).title("Animations"))
                    .style(Style::default().fg(Color::White));
                f.render_widget(list, chunks[1]);

                // Instructions
                let info = match mode {
                    AppMode::Browse => {
                        if selected_idx < animations.len() {
                            format!(
                                "Selected: {} | Frame: {} | Speed: {}ms",
                                animations[selected_idx].name,
                                animations[selected_idx].current(),
                                animations[selected_idx].speed
                            )
                        } else {
                            "Selected: [+ Add New Animation] | Press ENTER to create".to_string()
                        }
                    }
                    AppMode::Creating(ref frames) => {
                        format!("Frames entered: {} | Speed: 200ms (default)", frames.len())
                    }
                };
                let info = Paragraph::new(info)
                    .alignment(Alignment::Center)
                    .style(Style::default().fg(Color::Yellow));
                f.render_widget(info, chunks[2]);
            })
            .map_err(|e| CliError::Generic(e.to_string()))?;

        // Update animation frame if playing
        if let Some(idx) = playing_idx {
            if idx < animations.len() {
                frame_count += 1;
                if frame_count * 50 >= animations[idx].speed {
                    animations[idx].next_frame();
                    frame_count = 0;
                }
            }
        }

        // Handle input
        if event::poll(Duration::from_millis(50)).map_err(|e| CliError::Generic(e.to_string()))? {
            if let Event::Key(key) = event::read().map_err(|e| CliError::Generic(e.to_string()))? {
                match mode {
                    AppMode::Browse => {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Esc => break,
                            KeyCode::Char('a') => {
                                // Exit TUI to get input
                                disable_raw_mode().ok();
                                execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();

                                let frames = prompt_for_frames()?;
                                if !frames.is_empty() {
                                    let name = format!("Custom ({})", frames.len());
                                    animations.push(AnimationBuilder::from_strings(&name, frames, 200));
                                    selected_idx = animations.len() - 1;
                                }

                                // Re-enable TUI
                                enable_raw_mode().map_err(|e| CliError::Generic(e.to_string()))?;
                                execute!(terminal.backend_mut(), EnterAlternateScreen)
                                    .map_err(|e| CliError::Generic(e.to_string()))?;
                                terminal.clear().ok();
                                mode = AppMode::Browse;
                            }
                            KeyCode::Up => {
                                selected_idx = selected_idx.saturating_sub(1);
                                playing_idx = None;
                            }
                            KeyCode::Down => {
                                selected_idx = (selected_idx + 1).min(animations.len());
                                playing_idx = None;
                            }
                            KeyCode::Enter => {
                                if selected_idx < animations.len() {
                                    if playing_idx == Some(selected_idx) {
                                        playing_idx = None;
                                    } else {
                                        playing_idx = Some(selected_idx);
                                        animations[selected_idx].current_frame = 0;
                                        frame_count = 0;
                                    }
                                } else if selected_idx == animations.len() {
                                    // Trigger "Add New Animation"
                                    // Exit TUI to get input
                                    disable_raw_mode().ok();
                                    execute!(terminal.backend_mut(), LeaveAlternateScreen).ok();

                                    let frames = prompt_for_frames()?;
                                    if !frames.is_empty() {
                                        let name = format!("Custom ({})", frames.len());
                                        animations.push(AnimationBuilder::from_strings(&name, frames, 200));
                                        selected_idx = animations.len() - 1;
                                    }

                                    // Re-enable TUI
                                    enable_raw_mode().map_err(|e| CliError::Generic(e.to_string()))?;
                                    execute!(terminal.backend_mut(), EnterAlternateScreen)
                                        .map_err(|e| CliError::Generic(e.to_string()))?;
                                    terminal.clear().ok();
                                }
                            }
                            _ => {}
                        }
                    }
                    AppMode::Creating(_) => {
                        // Not used anymore
                    }
                }
            }
        }
    }

    Ok(())
}

fn prompt_for_frames() -> Result<Vec<String>, CliError> {
    // Clear screen
    print!("\x1B[2J\x1B[H");

    println!("\n╔════════════════════════════════════════════════════════════════╗");
    println!("║               ADD NEW ANIMATION                              ║");
    println!("╚════════════════════════════════════════════════════════════════╝\n");
    println!("Paste frames separated by spaces, all on one line.");
    println!("Example: ⢄ ⢂ ⢁ ⡁ ⡈ ⡐ ⡠\n");
    println!("Enter frames: ");

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|e| CliError::Generic(e.to_string()))?;

    let frames: Vec<String> = input
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    for frame in &frames {
        println!("  ✓ Added frame: {}", frame);
    }

    println!("\n✓ Added {} frames total.", frames.len());
    println!("\nReturning to animation builder...\n");
    thread::sleep(Duration::from_millis(1000));

    Ok(frames)
}

enum AppMode {
    Browse,
    Creating(Vec<String>),
}
