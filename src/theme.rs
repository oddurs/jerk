use ratatui::style::{Color, Modifier, Style};

#[derive(Clone, Copy)]
pub struct Theme {
    pub text: Color,
    pub muted: Color,
    pub faint: Color,
    pub accent: Color,
    pub secondary: Color,
    pub good: Color,
    pub warn: Color,
    pub bad: Color,
    pub planning: Color,
    pub border: Color,
    pub monochrome: bool,
}

impl Theme {
    pub fn terminal() -> Self {
        let monochrome = std::env::var_os("NO_COLOR").is_some();
        if monochrome {
            return Self::mono();
        }
        // Named ANSI colours are deliberate: Ghostty and the parent terminal
        // substitute their configured palette. Reset preserves its background.
        Self {
            text: Color::Reset,
            muted: Color::Gray,
            faint: Color::DarkGray,
            accent: Color::Blue,
            secondary: Color::Cyan,
            good: Color::Green,
            warn: Color::Yellow,
            bad: Color::Red,
            planning: Color::Magenta,
            border: Color::DarkGray,
            monochrome: false,
        }
    }

    fn mono() -> Self {
        Self {
            text: Color::Reset,
            muted: Color::Reset,
            faint: Color::Reset,
            accent: Color::Reset,
            secondary: Color::Reset,
            good: Color::Reset,
            warn: Color::Reset,
            bad: Color::Reset,
            planning: Color::Reset,
            border: Color::Reset,
            monochrome: true,
        }
    }

    pub fn selected(self) -> Style {
        // Foreground-only selection keeps the terminal palette authoritative.
        // Reverse video creates a bright slab on dark or tinted backgrounds.
        Style::default()
            .fg(self.accent)
            .add_modifier(Modifier::BOLD)
    }

    pub fn score(self, score: u16) -> Color {
        match score {
            75..=100 => self.good,
            50..=74 => self.warn,
            _ => self.bad,
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::style::Modifier;

    use super::Theme;

    #[test]
    fn selection_never_paints_over_the_terminal_palette() {
        let style = Theme::terminal().selected();

        assert!(style.bg.is_none());
        assert!(!style.add_modifier.contains(Modifier::REVERSED));
    }
}
