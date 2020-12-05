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

*install a pacakge with nightly*

`rustman -S +nightly $package`

*remove a pacakge*

`rustman -R $package`

*show installed pacakges*

`rustman --list`

## Example
`rustman repl`

<img src="./rustman.png" width="70%" height="60%">

## Releases
Automatic releases by github actions are uploaded here https://github.com/sigmaSd/rustman/releases

## [Changelog](./CHANGELOG.md)
