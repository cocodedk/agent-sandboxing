//! Which homes are checked and which opens are foreign.

use super::{Credentials, accounts, homes};

const PASSWD: &str = "root:x:0:0:root:/root:/bin/bash
daemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin
agent:x:1000:1000:Agent,,,:/home/agent/:/bin/bash
broken line
";

fn credentials() -> Credentials {
    Credentials::new(&homes(&accounts(PASSWD)))
}

#[test]
fn root_and_login_accounts_only() {
    let accounts = accounts(PASSWD);
    assert_eq!(homes(&accounts), ["/root", "/home/agent/"]);
    assert_eq!((accounts[1].uid, accounts[1].gid), (1000, 1000));
}

#[test]
fn a_tool_reading_its_own_folder_is_no_alert() {
    let credentials = credentials();
    assert!(!credentials.foreign("ssh", "/home/agent/.ssh/id_ed25519"));
    assert!(!credentials.foreign("gh", "/home/agent/.config/gh/hosts.yml"));
}

#[test]
fn another_program_is_an_alert() {
    let credentials = credentials();
    assert!(credentials.foreign("cat", "/home/agent/.ssh/id_ed25519"));
    assert!(credentials.foreign("ssh", "/root/.config/gh/hosts.yml"));
    assert!(credentials.foreign("cat", "/home/agent/.ssh")); // the folder itself
}

#[test]
fn a_folder_without_owner_alerts_on_any_reader() {
    assert!(credentials().foreign("aws", "/home/agent/.aws/credentials"));
}

#[test]
fn look_alike_and_unchecked_paths_are_no_alert() {
    let credentials = credentials();
    assert!(!credentials.foreign("cat", "/home/agent/.sshx/key"));
    assert!(!credentials.foreign("cat", "/usr/sbin/.ssh/key")); // a system account's home
    assert!(!credentials.foreign("cat", "/home/agent/notes.txt"));
}
