# Rust GUI Typing Tutor Demo (using Iced)

> A very basic proof-of-concept GUI typing tutor application built with Rust and the [Iced](https://github.com/iced-rs/iced) GUI library.

## ⚠️ Status / Disclaimer ⚠️

**This project is a minimal demonstration and NOT a fully functional typing tutor.**

It was created as a programming exercise to illustrate basic GUI concepts in Rust with the Iced library. It lacks many core features expected of a real typing tutor application.

## Features

### Current Features (Basic Proof-of-Concept)

* Displays a hardcoded sample text passage.
* A simple GUI window using the Iced framework.
* A "Start Typing" / "Restart Test" button to begin/reset.
* A text input field for the user to type into.
* Basic timing: Starts a timer when the test begins and stops when the input length matches the sample text length.
* Post-test calculation and display of:
    * Time taken (seconds)
    * Accuracy (%)
    * Net WPM (Words Per Minute, based on standard 5 chars/word and correct characters)

### Missing / Planned Features

This demo **lacks** many essential features, including but not limited to:

* Real-time feedback (character-by-character validation and error highlighting).
* Visual keyboard display.
* Loading different lessons or texts (currently uses one hardcoded string).
* User profiles and progress saving.
* More sophisticated WPM/accuracy calculations (e.g., handling backspace, error penalties).
* Proper handling of test completion (e.g., detecting Enter key instead of just matching length).
* Advanced UI elements and better layout.
* Configuration options.

## Technology

* **Language:** [Rust](https://www.rust-lang.org/) (Edition 2021)
* **GUI Library:** [Iced](https://github.com/iced-rs/iced) (~0.12)

## Prerequisites

* **Rust Toolchain:** Install Rust via [rustup](https://rustup.rs/).
* **System Dependencies:** The Iced library relies on native dependencies for rendering, windowing, etc. You may need to install system libraries depending on your Operating System (Linux, macOS, Windows). Please refer to the [Iced documentation](https://docs.iced.rs/getting-started/installation) for platform-specific requirements (e.g., `libfontconfig-dev`, `libgtk-3-dev` on Debian/Ubuntu).

## Getting Started

### Installation

1.  Clone this repository (or download the source code):
    ```bash
    # Replace with the actual URL if you put this on GitHub/GitLab etc.
    git clone https://github.com/rmpr/keystroke.git
    cd keystroke
    ```

### Building

2.  Build the project using Cargo:
    ```bash
    cargo build
    ```
    *(This will download dependencies and compile the code. The first build might take some time.)*

### Running

3.  Run the compiled application:
    ```bash
    cargo run
    ```

## How to Use

1.  Launch the application (`cargo run`).
2.  A window will appear displaying the sample text.
3.  Click the "Start Typing!" button.
4.  The timer starts, and you can type in the text input field below the sample text.
5.  Try to type the sample text exactly.
6.  Once the number of characters you've typed matches the length of the sample text, the test will automatically stop, and the results (Time, Accuracy, WPM) will be displayed.
7.  You can click "Restart Test" to try again.

## Future Development Ideas

If you wanted to expand this demo:

* Implement real-time character validation and visual feedback (e.g., coloring text).
* Use a more robust method to end the test.
* Load lesson text from external files (e.g., `.txt`, `.json`).
* Add multiple lessons and a way to select them.
* Introduce user profiles to track progress over time.
* Improve the UI/UX design.
* Add a visual keyboard representation.

## License

This project currently does not have a license file. If you plan to distribute or build upon this code, it's recommended to choose an open-source license (e.g., MIT License or Apache License 2.0) and add a `LICENSE` file to the repository.
