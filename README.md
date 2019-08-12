# rustman
Cross platfrom package manager

## Description
Search and install rust binaires

## Usage
*Update all local packages*

`rustman`

*Search and install interactively a specific pacakge*

`rustman $package`

*install a pacakge*

`rustman -S $package`

*remove a pacakge*

`rustman -R $package`

*show installed pacakges*

`rustman --show-installed`

**flags:**

- `--update-database` -> force database update
- `--offline` -> skip database update

## How It works

rustman will download metadata about all available crates once per day, or if forced with `--update-database` flag

Unfortunately crates.io api doesn't expose if a crate is a binary or not, so the for now rustman will search all crates, and each time a non binary crate is downloaded it will add it to a blacklist so it doesn't appear again in search results

## Example
`rustman repl`

<img src="./rustman.png" width="70%" height="60%">

## [Changelog](./CHANGELOG.md)
