**0.5.3**
- Add the ability to specify the toolchain for the install, example: `rustman -S +nightly program`

**0.5.2**
- sync error reporting

**0.5.1**
 - fix regression (is_bin was being ignored)
 - add bot url
 - more error handling

**0.5.0**
- Port to async (Tokio runtime + reqwest)
- Clean up code, handle errors, remove unnecessary functions, etc..

**0.4.2**
- `rustman -S` now sends correctly the args to cargo install (so for exp, `rustman -S --git $repo_url` now works)

**0.4.1**
- Add the ability do download binaries using `--custom-url` arg, note you must also specify the version  with `--version` so `.crates.toml` can be updated
- Updated dependencies

**0.4.0**
- Yank all 0.3* versions and reset to 0.2.3 becuase of crates.io crawling policy

**0.3.3**
- Make `show-installed` a cmd instead of a flag

**0.3.2**
- Improve search logic (multiple arguments are now intersected)

**0.3.1**
- Add offline flag

**0.3.0**
- Bypass cargo search limitation of 100 hit, rustman now searches all crates

**0.2.3**
- Add `-S` `-R` `--installed args`

**0.2.2**
- Use unchained crate
- Handle the case where all pkgs are uptodate

**0.2.1**
- Add colors to update-all
- Refactor code

**0.2.0**
- add update-all behaviour, and make it the default when calling `rustman` with no args

**0.1.9**

- Better handling of errors
- Fix a bug in handling input

**0.1.8**

- Handle multi-item input

**0.1.7**

- Make description optional (fixes panic)
- Handle no input error

**0.1.6**

- Use std::thread instead of rayon for atleast 4x speedup

**0.1.5**

- Fix some windows specific bugs

**0.1.4**

- Add a progress bar

**0.1.3**

- Use termcolor instead of colord for better crossplatform support
