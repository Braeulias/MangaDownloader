# Manga Downloader

Manga Downloader is a Rust-based program for downloading manga chapters from online providers, converting them into PDFs, and saving them locally for offline reading. This README provides an overview of the features, usage, and prerequisites for setting up and running the program.

## Features

- **Manga Search**: Search for manga by title using supported providers (currently Mangadex).
- **Chapter Selection**: Select specific chapters to download.
- **Parallel Image Download**: Efficiently fetch manga pages with parallel requests.
- **PDF Generation**: Convert downloaded manga chapters into PDF format.
- **User-Friendly Interface**: Interactive prompts to guide users through the process.

## Prerequisites

1. **Rust**: Ensure you have Rust installed. If not, download it from [Rust's official website](https://www.rust-lang.org/).
2. **Dependencies**: The program uses the following Rust crates:
    - `directories`
    - `futures`
    - `indicatif`
    - `inquire`
    - `lopdf`
    - `reqwest`
    - `serde_json`
    - `tokio`

   Install these dependencies using `cargo`.

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/Braeulias/MangaDownloader.git
   cd manga-downloader
   ```

2. Build the project:
   ```bash
   cargo build --release
   ```

3. Run the executable:
   ```bash
   cargo run --release
   ```

## Usage

1. **Select Manga Provider**: Choose a supported provider (e.g., Mangadex).
2. **Search Manga**: Enter the title of the manga you wish to download.
3. **Select Manga**: Choose a manga from the search results.
4. **Select Chapters**: Pick the chapters you want to download.
5. **Download and Convert**: The program downloads images and generates PDFs in your Downloads directory.

## Directory Structure

The PDFs are saved in a subdirectory named after the manga title within your system's Downloads folder. The program ensures safe file naming by replacing invalid characters with underscores (`_`).

## Example Workflow

1. **Input**: Search for "One Piece."
2. **Select**: Choose "One Piece" from the search results.
3. **Pick Chapters**: Select chapters 1, 2, and 3.
4. **Output**: PDFs named `Chapter_1_ChapterName.pdf`, `Chapter_2_ChapterName.pdf`, and `Chapter_3_ChapterName.pdf.pdf` appear in the `Downloads/One_Piece` directory.

## Known Issues

- **Limited Provider Support**: Only Mangadex is currently supported. Additional providers are a planned feature.
- **Error Handling**: The program does not handle interruptions or download failures robustly. This will be added very soon, i kinda dont want to do it right now.
- **Performance**: Large downloads may take significant time, depending on network speed.

## Contributing

Contributions are welcome! To contribute:

1. Fork the repository.
2. Create a feature branch:
   ```bash
   git checkout -b feature-name
   ```
3. Commit your changes:
   ```bash
   git commit -m "Add new feature"
   ```
4. Push to your branch:
   ```bash
   git push origin feature-name
   ```
5. Create a pull request.


## Acknowledgments

- **Mangadex**: For providing an API to access manga data.
- **Rust Crate Developers**: For the excellent libraries used in this project.

---

Enjoy your offline manga reading experience!

