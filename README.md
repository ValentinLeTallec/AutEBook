# AutE-Book

The goal of AutE-Book is to automatically e-books of webnovels up to date with the latest chapters posted.

Currently only e-books from royalroad.com are supported.

## Roadmap

- [x] Update all already existing e-books in a folder (levrage [FanFicFare](https://github.com/JimmXinu/FanFicFare) to update each e-book individually)
- [x] Display a progress bar
- [ ] Manage STUBs (novel that get their beginning truncated because of Kindle Unlimited exclusivity policie)
- [ ] Add an exclude pattern
- [ ] .gitignore style file
  - [ ] Add support for such file
  - [ ] Propose to generated such file pre-filed with unsupported files
- [ ] Config file

-----

- [ ] Add new e-books from url
- [ ] Auto update e-books by using RSS feeds to check for updates (ideally as AutE-Book would be running as a deamon in that case)
- [ ] Send notifications for newly updated books

## Dependencies

[FanFicFare](https://github.com/JimmXinu/FanFicFare) and rustup must be installed.
