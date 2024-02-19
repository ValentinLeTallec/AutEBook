# rr-to-epub

> [!NOTE]  
> All stories are the property of their respective authors. This application is not affiliated with Royal Road in any way and makes no attempt to claim credit for any stories downloaded. **If anyone working at Royal Road wants this tool taken down, please reach out to me at the email linked to my Github and I will immediately comply.**

A small application to convert a [Royal Road](https://www.royalroad.com/) story to an `.epub` file, compatible with readers such as Kindle or Calibre. Motivated by me wanting to read some stories on my Kindle when I'm without access to the Royal Road web service.

## Install

This tool is written in Rust. To install it, first install [Rust](https://www.rust-lang.org/tools/install), then run the following command:

```
cargo install --git https://github.com/isaac-mcfadyen/rr-to-epub
```

## Usage

After install, download a book by running the following command, replacing the ID with the book ID. (Book IDs can be found in the URL of the book, e.g. `https://www.royalroad.com/fiction/<book-id>/`)

```sh
rr-to-epub --book-id <book-id> ./file-to-output.epub
```

Full help can be found by running `rr-to-epub --help`.
