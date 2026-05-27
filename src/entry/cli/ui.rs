use console::{Color, Style};

pub struct CliUi {
    primary: Style,
    accent: Style,
    muted: Style,
    error: Style,
}

impl Default for CliUi {
    fn default() -> Self {
        Self::new()
    }
}

impl CliUi {
    pub fn new() -> Self {
        Self {
            primary: Style::new().fg(Color::Color256(63)).bold(),
            accent: Style::new().fg(Color::Color256(75)).bold(),
            muted: Style::new().fg(Color::Color256(245)),
            error: Style::new().fg(Color::Red).bold(),
        }
    }

    pub fn banner(&self) {
        let banner_style = Style::new().fg(Color::Color256(99)).bold();
        let banner = r" ________ ___  ___  ________  ________  _____ ______   ___  ________   ________     
|\  _____\\  \|\  \|\_____  \|\_____  \|\   _ \  _   \|\  \|\   ___  \|\   ____\    
\ \  \__/\ \  \\\  \\|___/  /|\|___/  /\ \  \\\__\ \  \ \  \ \  \\ \  \ \  \___|    
 \ \   __\\ \  \\\  \   /  / /    /  / /\ \  \\|__| \  \ \  \ \  \\ \  \ \  \  ___  
  \ \  \_| \ \  \\\  \ /  /_/__  /  /_/__\ \  \    \ \  \ \  \ \  \\ \  \ \  \|\  \ 
   \ \__\   \ \_______\\________\\________\ \__\    \ \__\ \__\ \__\\ \__\ \_______\
    \|__|    \|_______|\|_______|\|_______|\|__|     \|__|\|__|\|__| \|__|\|_______|
                                                                                    
                                                                                    
                                                                                    ";
        println!("{}", banner_style.apply_to(banner));
        println!();
    }

    pub fn question(&self, label: &str) -> String {
        self.accent.apply_to(label).to_string()
    }

    pub fn divider(&self) {
        println!(
            "{}",
            self.muted
                .apply_to("----------------------------------------")
        );
    }

    pub fn info(&self, message: &str) {
        println!("{}", self.muted.apply_to(message));
    }

    pub fn success(&self, message: &str) {
        println!("{}", self.primary.apply_to(message));
    }

    pub fn warn(&self, message: &str) {
        println!("{}", self.accent.apply_to(message));
    }

    pub fn error(&self, message: &str) {
        eprintln!("{}", self.error.apply_to(message));
    }
}
