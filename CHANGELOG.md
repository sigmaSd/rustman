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
