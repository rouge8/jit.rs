use lazy_static::lazy_static;
use std::collections::HashMap;
use std::io;
use std::io::Write;
use std::process::{Child, Command, Stdio};

const PAGER_CMD: &str = "less";

lazy_static! {
    static ref PAGER_ENV: HashMap<&'static str, &'static str> =
        HashMap::from([("LESS", "FRX"), ("LV", "-c"),]);
}

pub struct Pager {
    process: Child,
}

impl Pager {
    pub fn new(env: &HashMap<String, String>) -> Self {
        // GIT_PAGER || PAGER || PAGER_CMD
        let cmd = match (env.get("GIT_PAGER"), env.get("PAGER")) {
            (Some(git_pager), _) => git_pager.to_string(),
            (_, Some(pager)) => pager.to_string(),
            _ => PAGER_CMD.to_string(),
        };

        // Merge `env` with `PAGER_ENV`
        let mut env = env.clone();
        for (key, val) in PAGER_ENV.iter() {
            env.insert(key.to_string(), val.to_string());
        }

        let p = Command::new(cmd)
            .envs(&env)
            .stdin(Stdio::piped())
            .spawn()
            .unwrap();

        Pager { process: p }
    }
}

impl Write for Pager {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.process.stdin.as_mut().unwrap().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.process.stdin.as_mut().unwrap().flush()
    }
}

impl Drop for Pager {
    fn drop(&mut self) {
        self.process.wait().unwrap();
    }
}
