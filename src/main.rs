use iced::widget::{button, column, container, text, text_input, Column};
use once_cell::sync::Lazy;

static INPUT_ID: Lazy<text_input::Id> = Lazy::new(text_input::Id::unique);
use iced::{executor, Application, Command, Element, Length, Settings, Subscription, Theme};
use std::time::Instant;

// --- Main Application State ---
struct TypingTutor {
    status: Status,
    sample_text: String,
    current_input: String,
    start_time: Option<Instant>,
    results: Option<TypingResult>,
}

// --- Application Status ---
#[derive(Debug, Clone, PartialEq)]
enum Status {
    Ready,
    Typing,
    Finished,
}

// --- Calculation Result ---
#[derive(Debug, Clone)]
struct TypingResult {
    wpm: f64,
    accuracy: f64,
    duration: f64,
}

// --- Messages to update state ---
// These are triggered by user interactions or other events
#[derive(Debug, Clone)]
enum Message {
    StartTyping,
    InputChanged(String),
    FocusNext,
    FocusPrevious,
    // Could add Reset, LoadLesson, etc. in a real app
}

// --- Application Implementation ---
// The core logic resides here, handling state and UI updates
impl Application for TypingTutor {
    type Executor = executor::Default; // Specifies how async operations are run
    type Message = Message; // The type of messages this app handles
    type Theme = Theme; // The theme type (e.g., Light, Dark)
    type Flags = (); // Data passed during initialization (none here)

    // Initialize the application state
    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            Self {
                status: Status::Ready,
                // Using a shorter text for this basic GUI demo
                sample_text: "the quick brown fox".to_string(),
                current_input: "".to_string(),
                start_time: None,
                results: None,
            },
            Command::none(), // No initial commands to run
        )
    }

    // Sets the window title
    fn title(&self) -> String {
        String::from("Rust GUI Typing Tutor - Iced Demo")
    }

    // Handles messages and updates the application state
    fn update(&mut self, message: Message) -> Command<Message> {
        // Tab navigation is allowed regardless of the current status.
        match message {
            Message::FocusNext => return iced::widget::focus_next(),
            Message::FocusPrevious => return iced::widget::focus_previous(),
            _ => {}
        }

        match self.status {
            Status::Ready => {
                if let Message::StartTyping = message {
                    self.status = Status::Typing;
                    self.current_input.clear();
                    self.results = None;
                    self.start_time = None;
                    return text_input::focus(INPUT_ID.clone());
                }
            }
            Status::Typing => {
                if let Message::InputChanged(value) = message {
                    // Start the timer on the very first keystroke
                    if self.start_time.is_none() {
                        self.start_time = Some(Instant::now());
                    }
                    self.current_input = value;

                    // VERY basic finish condition: input reaches sample length.
                    // A real app needs a better way (e.g., detect Enter press, timeout, finish button).
                    // Also lacks real-time validation/highlighting.
                    if self.current_input.len() >= self.sample_text.len() {
                        // Truncate if user typed more (simple handling)
                        self.current_input =
                            self.current_input[..self.sample_text.len()].to_string();
                        self.status = Status::Finished;
                        if let Some(start_time) = self.start_time {
                            let duration = start_time.elapsed();
                            let results = calculate_results(
                                &self.sample_text,
                                &self.current_input,
                                duration.as_secs_f64(),
                            );
                            self.results = Some(results);
                        }
                    }
                }
            }
            Status::Finished => {
                // Allow restarting the test
                if let Message::StartTyping = message {
                    self.status = Status::Typing;
                    self.current_input.clear();
                    self.results = None;
                    self.start_time = None;
                    return text_input::focus(INPUT_ID.clone());
                }
            }
        }
        Command::none() // No async commands returned by default
    }

    // Defines the UI layout based on the current state
    fn view(&self) -> Element<Message> {
        // Use a Column layout to stack widgets vertically
        let mut col = Column::new()
            .spacing(10)
            .padding(20)
            .align_items(iced::Alignment::Center);

        // --- Display Sample Text ---
        col = col.push(text("Type the following text:").size(20));
        // A real app would use better text rendering (e.g., highlighting typed parts)
        col = col.push(text(&self.sample_text).size(24));

        // --- Display Input Field ---
        // The placeholder changes based on state
        let placeholder = match self.status {
            Status::Ready => "Click 'Start Typing!' to begin",
            Status::Typing => "Start typing here...",
            Status::Finished => "Test finished!",
        };

        let input_field = text_input(placeholder, &self.current_input)
            .id(INPUT_ID.clone())
            .padding(10);

        // Only allow input changes when in Typing state
        let active_input_field = if self.status == Status::Typing {
            input_field.on_input(Message::InputChanged)
        } else {
            input_field // Effectively read-only as on_input is missing
        };
        col = col.push(active_input_field);

        // --- Display Button ---
        let (button_text, button_message) = match self.status {
            Status::Ready => ("Start Typing!", Some(Message::StartTyping)),
            Status::Typing => ("Typing...", None), // Disable button press while typing
            Status::Finished => ("Restart Test", Some(Message::StartTyping)),
        };

        col = col.push(
            button(text(button_text).horizontal_alignment(iced::alignment::Horizontal::Center))
                .padding(10)
                .width(Length::Fixed(150.0)) // Give button fixed width
                .on_press_maybe(button_message), // `on_press_maybe` handles Option<Message>
        );

        // --- Display Results Area ---
        if let Some(results) = &self.results {
            // Only show results when finished
            if self.status == Status::Finished {
                col = col.push(
                    text("--- Results ---")
                        .size(20)
                        .style(iced::theme::Text::Default),
                ); // Use theme color
                col = col.push(text(format!("Time: {:.2} s", results.duration)));
                col = col.push(text(format!("Accuracy: {:.2}%", results.accuracy)));
                col = col.push(text(format!("WPM: {:.2}", results.wpm)));
            }
        }

        // --- Final Layout ---
        // Wrap the column in a container for centering and filling space
        container(col)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into() // Convert to Element
    }

    // Use the default theme (Light)
    fn theme(&self) -> Self::Theme {
        Theme::default()
    }

    // Listen for Tab / Shift+Tab key presses globally to move keyboard focus
    // between focusable widgets.
    fn subscription(&self) -> Subscription<Message> {
        iced::event::listen_with(|event, status| {
            // Only react to events that haven't already been consumed by a widget.
            if status != iced::event::Status::Ignored {
                return None;
            }
            if let iced::Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Tab),
                modifiers,
                ..
            }) = event
            {
                if modifiers.shift() {
                    Some(Message::FocusPrevious)
                } else {
                    Some(Message::FocusNext)
                }
            } else {
                None
            }
        })
    }
}

// --- Helper function for calculations (same logic as the console version) ---
fn calculate_results(sample_text: &str, user_input: &str, duration_secs: f64) -> TypingResult {
    let typed_chars_count = user_input.chars().count();
    let mut correct_chars_count = 0;
    let mut sample_iter = sample_text.chars();

    // Simple character-by-character comparison
    for typed_char in user_input.chars() {
        if let Some(sample_char) = sample_iter.next() {
            if typed_char == sample_char {
                correct_chars_count += 1;
            }
        } else {
            break; // Stop if user input exceeds sample text length
        }
    }

    let accuracy = if typed_chars_count > 0 {
        (correct_chars_count as f64 / typed_chars_count as f64) * 100.0
    } else {
        0.0
    };

    // Standard WPM calculation (Net WPM based on correct characters)
    let wpm = if duration_secs > 0.0 {
        (correct_chars_count as f64 / 5.0) / (duration_secs / 60.0)
    } else {
        0.0
    };

    TypingResult {
        wpm,
        accuracy,
        duration: duration_secs,
    }
}

// --- Main function to run the Iced application ---
pub fn main() -> iced::Result {
    // Set window settings if needed (size, etc.)
    let settings = Settings {
        window: iced::window::Settings {
            // Corrected line: Construct iced::Size explicitly
            size: iced::Size::new(600.0, 400.0), // Set initial window size using floats
            ..Default::default()
        },
        ..Default::default()
    };

    // Run the Iced application
    TypingTutor::run(settings)
}
