use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, LineGauge, List, ListItem, ListState, Paragraph, Sparkline,
    Wrap,
};

use crate::app::{App, Tab};
use crate::model::{Project, SignalKind};
use crate::theme::Theme;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    SelectTab(Tab),
    SelectProject(usize),
    Analyze,
    Payload,
    Refresh,
    Sort,
    Open,
    Help,
    Quit,
    CloseOverlay,
}

const SIDEBAR_WIDTH: u16 = 28;

fn shell_areas(area: Rect) -> [Rect; 4] {
    Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(12),
        Constraint::Length(1),
    ])
    .areas(area)
}

fn tab_labels() -> [String; 6] {
    Tab::ALL.map(|tab| format!(" {} {} ", tab as usize + 1, tab.label().to_lowercase()))
}

pub fn mouse_action(app: &App, area: Rect, column: u16, row: u16) -> Option<MouseAction> {
    if app.show_help || app.show_ai_payload {
        return Some(MouseAction::CloseOverlay);
    }
    let shell = shell_areas(area);
    if row >= shell[1].y && row < shell[1].y + shell[1].height {
        return tab_at(shell[1], column).map(MouseAction::SelectTab);
    }
    if row == shell[3].y {
        return footer_action_at(app.tab, area.width, column);
    }
    let has_sidebar = app.tab != Tab::Portfolio && shell[2].width >= 96 && app.visible_count() > 1;
    if has_sidebar && column < shell[2].x + SIDEBAR_WIDTH {
        let inner_top = shell[2].y.saturating_add(1);
        let inner_bottom = shell[2].y + shell[2].height;
        if row >= inner_top && row < inner_bottom {
            let visible_rows = usize::from(shell[2].height.saturating_sub(1)).max(1);
            let offset = project_list_offset(app.selected, app.visible_count(), visible_rows);
            let index = offset + usize::from(row - inner_top);
            if index < app.visible_count() {
                return Some(MouseAction::SelectProject(index));
            }
        }
    }
    None
}

fn tab_at(area: Rect, column: u16) -> Option<Tab> {
    let labels = tab_labels();
    let mut start = area.x.saturating_add(1);
    for (tab, label) in Tab::ALL.into_iter().zip(labels) {
        let end = start.saturating_add(label.chars().count() as u16);
        if column >= start && column < end {
            return Some(tab);
        }
        start = end;
    }
    None
}

fn footer_action_at(tab: Tab, width: u16, column: u16) -> Option<MouseAction> {
    let controls = footer_controls(tab, width);
    let start = width.saturating_sub(controls.chars().count() as u16);
    if column < start {
        return None;
    }
    let local = usize::from(column.saturating_sub(start));
    [
        ("a analyze", MouseAction::Analyze),
        ("p payload", MouseAction::Payload),
        ("s sort", MouseAction::Sort),
        ("r/R refresh", MouseAction::Refresh),
        ("r refresh", MouseAction::Refresh),
        ("o open", MouseAction::Open),
        ("? help", MouseAction::Help),
        ("q quit", MouseAction::Quit),
    ]
    .into_iter()
    .find_map(|(needle, action)| {
        let offset = controls.find(needle)?;
        (local >= offset && local < offset + needle.chars().count()).then_some(action)
    })
}

pub fn draw(frame: &mut Frame<'_>, app: &App, theme: Theme) {
    let area = frame.area();

    if area.width < 72 || area.height < 16 {
        draw_too_small(frame, theme, area);
        return;
    }

    let shell = shell_areas(area);
    draw_header(frame, app, theme, shell[0]);
    draw_tabs(frame, app, theme, shell[1]);

    if app.visible_count() == 0 {
        draw_empty(frame, app, theme, shell[2]);
    } else if app.tab == Tab::Portfolio || shell[2].width < 96 || app.visible_count() == 1 {
        draw_detail(frame, app, theme, shell[2]);
    } else {
        let columns = Layout::horizontal([Constraint::Length(SIDEBAR_WIDTH), Constraint::Min(60)])
            .spacing(1)
            .split(shell[2]);
        draw_projects(frame, app, theme, columns[0]);
        draw_detail(frame, app, theme, columns[1]);
    }
    draw_footer(frame, app, theme, shell[3]);

    if app.show_ai_payload {
        draw_ai_payload(frame, app, theme, area);
    }
    if app.show_help {
        draw_help(frame, theme, area);
    }
}

fn draw_header(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let current = app.current();
    let title = Line::from(vec![
        Span::styled(
            format!(" {}", app.dashboard.brand),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("  {}", app.dashboard.workspace),
            Style::default().fg(theme.faint),
        ),
        Span::styled(
            current
                .map(|project| format!("  / {}", project.name))
                .unwrap_or_default(),
            Style::default().fg(theme.muted),
        ),
    ]);
    let right = current
        .map(|_| {
            if app.is_loading() {
                "◌ sync ".to_string()
            } else {
                "● live ".to_string()
            }
        })
        .unwrap_or_else(|| {
            if app.scanning {
                "◌ discovering ".into()
            } else {
                "○ no projects ".into()
            }
        });
    let chunks = Layout::horizontal([
        Constraint::Min(20),
        Constraint::Length(right.chars().count() as u16 + 1),
    ])
    .split(area);
    frame.render_widget(Paragraph::new(title), chunks[0]);
    let color = current.map(|_| theme.good).unwrap_or(theme.muted);
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        chunks[1],
    );
}

fn draw_tabs(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let mut spans = vec![Span::raw(" ")];
    for (tab, label) in Tab::ALL.into_iter().zip(tab_labels()) {
        let style = if tab == app.tab {
            theme.selected().add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().fg(theme.muted)
        };
        spans.push(Span::styled(label, style));
        spans.push(Span::raw(""));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

fn draw_projects(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let name_width = usize::from(area.width.saturating_sub(15)).max(4);
    let items = app
        .visible_projects()
        .map(|project| {
            let (mark, color) = score_mark(project.score.total, theme);
            let dirty = if project.git.dirty > 0 { " *" } else { "" };
            let age = project
                .git
                .last_commit_age_days
                .map(human_age)
                .unwrap_or_else(|| "—".into());
            let mut name = format!("{}{}", project.name, dirty);
            if name.chars().count() > name_width {
                name = name.chars().take(name_width.saturating_sub(1)).collect();
                name.push('…');
            }
            let padding = name_width.saturating_sub(name.chars().count());
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!("{mark}{:>3} ", project.score.total),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!("{name}{}", " ".repeat(padding)),
                    Style::default().fg(theme.text),
                ),
                Span::styled(
                    activity_strip(&project.git.activity, 3),
                    Style::default().fg(theme.secondary),
                ),
                Span::styled(format!(" {age:>4}"), Style::default().fg(theme.faint)),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items)
        .block(panel(
            if app.filter.is_empty() {
                format!(" PROJECTS · {} · {} ", app.projects.len(), app.sort.label())
            } else {
                format!(
                    " PROJECTS · {}/{} · {} ",
                    app.visible_count(),
                    app.projects.len(),
                    app.sort.label()
                )
            },
            theme,
            false,
        ))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD))
        .highlight_symbol("›");
    let visible_rows = usize::from(area.height.saturating_sub(1)).max(1);
    let offset = project_list_offset(app.selected, app.visible_count(), visible_rows);
    let mut state = ListState::default()
        .with_selected(Some(app.selected))
        .with_offset(offset);
    frame.render_stateful_widget(list, area, &mut state);
}

fn project_list_offset(selected: usize, count: usize, visible_rows: usize) -> usize {
    selected
        .saturating_add(1)
        .saturating_sub(visible_rows)
        .min(count.saturating_sub(visible_rows))
}

fn draw_detail(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    // Avoid stretching a handful of signals into enormous bordered boxes on
    // tall terminals. Unpainted space preserves the user's background.
    let area = Rect {
        height: area.height.min(28),
        ..area
    };
    if app.tab == Tab::Portfolio {
        draw_portfolio(frame, app, theme, area);
        return;
    }
    let Some(project) = app.current() else {
        return;
    };
    match app.tab {
        Tab::Overview => draw_overview(frame, project, theme, area),
        Tab::Git => draw_git(frame, project, theme, area),
        Tab::Delivery => draw_delivery(frame, project, theme, area),
        Tab::Cairn => draw_cairn(frame, project, theme, area),
        Tab::Portfolio => {}
        Tab::Insight => draw_ai(frame, app, project, theme, area),
    }
}

fn draw_ai(frame: &mut Frame<'_>, app: &App, project: &Project, theme: Theme, area: Rect) {
    let credential = app
        .ai_credential_source
        .as_deref()
        .map(|source| format!("key · {source}"))
        .unwrap_or_else(|| "key · missing".into());
    let stale = app.current_ai_is_stale();

    if app.is_ai_loading() {
        let lines = vec![
            Line::from(Span::styled(
                "Analyzing bounded project metrics…",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(vec![
                Span::styled(" MODEL  ", Style::default().fg(theme.faint)),
                Span::styled(app.ai_model.clone(), Style::default().fg(theme.text)),
            ]),
            Line::from(Span::styled(
                "The local dashboard remains responsive while OpenRouter works.",
                Style::default().fg(theme.muted),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines).wrap(Wrap { trim: true }).block(panel(
                " AI INSIGHT · WORKING ",
                theme,
                true,
            )),
            Rect {
                height: area.height.min(4),
                ..area
            },
        );
        return;
    }

    if let Some(report) = app.current_ai_report() {
        if area.height < 18 {
            let mut lines = vec![
                Line::from(Span::styled(
                    report.summary.clone(),
                    Style::default().fg(theme.text),
                )),
                Line::raw(""),
                Line::from(Span::styled(
                    "OBSERVATIONS",
                    Style::default()
                        .fg(theme.secondary)
                        .add_modifier(Modifier::BOLD),
                )),
            ];
            let observations = report
                .observations
                .iter()
                .take(2)
                .cloned()
                .collect::<Vec<_>>();
            lines.extend(ai_bullets(&observations, theme.secondary, theme));
            lines.push(Line::from(Span::styled(
                "RISKS",
                Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
            )));
            let risks = report.risks.iter().take(2).cloned().collect::<Vec<_>>();
            lines.extend(ai_bullets(&risks, theme.warn, theme));
            lines.push(Line::from(vec![
                Span::styled("NEXT  ", Style::default().fg(theme.good)),
                Span::styled(
                    report.next_action.clone(),
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
            ]));
            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: true }).block(panel(
                    format!(" AI BRIEF · {} ", project.name),
                    theme,
                    true,
                )),
                area,
            );
            return;
        }
        let rows = Layout::vertical([
            Constraint::Length(4),
            Constraint::Min(5),
            Constraint::Length(3),
        ])
        .split(area);
        let token_note = report
            .total_tokens
            .map(|tokens| format!(" · {tokens} tokens"))
            .unwrap_or_default();
        let state_note = if stale {
            " · metrics changed"
        } else {
            " · current"
        };
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(Span::styled(
                    report.summary.clone(),
                    Style::default().fg(theme.text),
                )),
                Line::from(Span::styled(
                    format!(" {}{token_note}{state_note}", report.model),
                    Style::default().fg(if stale { theme.warn } else { theme.faint }),
                )),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(format!(" AI BRIEF · {} ", project.name), theme, true)),
            rows[0],
        );
        let columns = Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)])
            .spacing(1)
            .split(rows[1]);
        frame.render_widget(
            Paragraph::new(ai_bullets(&report.observations, theme.secondary, theme))
                .wrap(Wrap { trim: true })
                .block(panel(" OBSERVATIONS ", theme, true)),
            columns[0],
        );
        frame.render_widget(
            Paragraph::new(ai_bullets(&report.risks, theme.warn, theme))
                .wrap(Wrap { trim: true })
                .block(panel(" RISKS ", theme, true)),
            columns[1],
        );
        frame.render_widget(
            Paragraph::new(vec![
                Line::from(vec![
                    Span::styled(" → ", Style::default().fg(theme.good)),
                    Span::styled(
                        report.next_action.clone(),
                        Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(Span::styled(
                    " a refresh analysis   p inspect exact payload",
                    Style::default().fg(theme.faint),
                )),
            ])
            .wrap(Wrap { trim: true })
            .block(panel(" HIGHEST-LEVERAGE NEXT ACTION ", theme, true)),
            rows[2],
        );
        return;
    }

    let error = app.ai_error.as_deref();
    let ready = error.is_none() && app.ai_credential_source.is_some();
    let lines = vec![
        Line::from(Span::styled(
            error.unwrap_or(if ready {
                "Ready when you are. Nothing has been sent."
            } else {
                "Add an OpenRouter key before requesting analysis."
            }),
            Style::default()
                .fg(if error.is_some() {
                    theme.bad
                } else {
                    theme.text
                })
                .add_modifier(Modifier::BOLD),
        )),
        field("Provider", "OpenRouter", theme),
        field("Model", &app.ai_model, theme),
        field("Credential", &credential, theme),
        Line::from(vec![
            Span::styled(" p ", theme.selected()),
            Span::styled(
                "preview the exact metrics-only JSON",
                Style::default().fg(theme.muted),
            ),
        ]),
        Line::from(vec![
            Span::styled(" a ", theme.selected()),
            Span::styled(
                "send it and generate a project brief",
                Style::default().fg(theme.muted),
            ),
        ]),
        Line::from(Span::styled(
            "No paths, URLs, descriptions, commit messages, source code, or Cairn item titles are included.",
            Style::default().fg(theme.faint),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: true }).block(panel(
            " AI INSIGHT · OPT-IN ",
            theme,
            true,
        )),
        Rect {
            height: area.height.min(9),
            ..area
        },
    );
}

fn ai_bullets(items: &[String], color: ratatui::style::Color, theme: Theme) -> Vec<Line<'static>> {
    items
        .iter()
        .map(|item| {
            Line::from(vec![
                Span::styled(" • ", Style::default().fg(color)),
                Span::styled(item.clone(), Style::default().fg(theme.text)),
            ])
        })
        .collect()
}

fn draw_portfolio(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let projects = app.visible_projects().collect::<Vec<_>>();
    let total = projects.len();
    let average = projects
        .iter()
        .map(|project| usize::from(project.score.total))
        .sum::<usize>()
        .checked_div(total)
        .unwrap_or_default();
    let active = projects
        .iter()
        .filter(|project| {
            matches!(
                project.profile.lifecycle,
                crate::model::Lifecycle::Active | crate::model::Lifecycle::Incubating
            )
        })
        .count();
    let wip = projects
        .iter()
        .filter_map(|project| project.cairn.as_ref())
        .map(|cairn| cairn.active)
        .sum::<usize>();

    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(7),
        Constraint::Min(0),
    ])
    .split(area);
    frame.render_widget(
        LineGauge::default()
            .block(panel(" PORTFOLIO EFFECTIVENESS ", theme, true))
            .ratio(average as f64 / 100.0)
            .filled_symbol("━")
            .unfilled_symbol("─")
            .filled_style(
                Style::default()
                    .fg(theme.score(average as u16))
                    .add_modifier(Modifier::BOLD),
            )
            .unfilled_style(Style::default().fg(theme.faint))
            .style(Style::default().fg(theme.muted))
            .label(match &app.portfolio {
                Some(github) => format!(
                    "{average} local average · {total} local · {} GitHub",
                    github.total
                ),
                None if app.portfolio_loading => {
                    format!("{average} local average · loading GitHub inventory…")
                }
                None => format!("{average} local average · {total} local projects"),
            }),
        rows[0],
    );

    let show_velocity = rows[1].width >= 100;
    let cards = if show_velocity {
        Layout::horizontal([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(rows[1])
    } else {
        Layout::horizontal([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(rows[1])
    };
    metric(
        frame,
        cards[0],
        "LOCAL",
        &total.to_string(),
        "checked out",
        theme.accent,
        theme,
    );
    metric(
        frame,
        cards[1],
        "GITHUB",
        &app.portfolio
            .as_ref()
            .map(|stats| stats.total.to_string())
            .unwrap_or_else(|| "—".into()),
        "source repos",
        theme.secondary,
        theme,
    );
    let active_card = if show_velocity {
        metric(
            frame,
            cards[2],
            "NEW · 30D",
            &app.portfolio
                .as_ref()
                .map(|stats| stats.created_30d.to_string())
                .unwrap_or_else(|| "—".into()),
            "repos created",
            theme.good,
            theme,
        );
        3
    } else {
        2
    };
    metric(
        frame,
        cards[active_card],
        "ACTIVE · 30D",
        &app.portfolio
            .as_ref()
            .map(|stats| stats.active_30d.to_string())
            .unwrap_or_else(|| active.to_string()),
        "pushed recently",
        theme.warn,
        theme,
    );
    metric(
        frame,
        cards[active_card + 1],
        "WIP",
        &wip.to_string(),
        "active Cairn",
        theme.planning,
        theme,
    );

    let columns = if rows[2].width >= 110 {
        Layout::horizontal([
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
            Constraint::Ratio(1, 5),
        ])
        .split(rows[2])
    } else if rows[2].width >= 100 {
        Layout::horizontal([
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
            Constraint::Ratio(1, 4),
        ])
        .split(rows[2])
    } else {
        Layout::horizontal([
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
            Constraint::Ratio(1, 3),
        ])
        .split(rows[2])
    };
    let lifecycle = [
        crate::model::Lifecycle::Incubating,
        crate::model::Lifecycle::Active,
        crate::model::Lifecycle::Maintenance,
        crate::model::Lifecycle::Complete,
        crate::model::Lifecycle::Paused,
        crate::model::Lifecycle::Archived,
    ]
    .into_iter()
    .map(|state| {
        let count = projects
            .iter()
            .filter(|project| project.profile.lifecycle == state)
            .count();
        let label = match state {
            crate::model::Lifecycle::Incubating => "INCUBATE",
            crate::model::Lifecycle::Active => "ACTIVE",
            crate::model::Lifecycle::Maintenance => "MAINTAIN",
            crate::model::Lifecycle::Complete => "COMPLETE",
            crate::model::Lifecycle::Paused => "PAUSED",
            crate::model::Lifecycle::Archived => "ARCHIVE",
        };
        compact_field(label, &count.to_string(), theme)
    })
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lifecycle).block(panel(" LIFECYCLE ", theme, true)),
        columns[0],
    );

    let coverage = [
        ("README", projects.iter().filter(|p| p.docs.readme).count()),
        (
            "LICENSE",
            projects.iter().filter(|p| p.docs.license).count(),
        ),
        ("CI", projects.iter().filter(|p| p.delivery.ci).count()),
        (
            "TESTS",
            projects.iter().filter(|p| p.delivery.tests).count(),
        ),
        (
            "CAIRN",
            projects.iter().filter(|p| p.cairn.is_some()).count(),
        ),
        ("SITE", projects.iter().filter(|p| p.site.source).count()),
    ]
    .into_iter()
    .map(|(name, count)| compact_field(name, &format!("{count}/{total}"), theme))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(coverage).block(panel(" COVERAGE ", theme, true)),
        columns[1],
    );

    let github = app.portfolio.as_ref();
    let cloud_only = github.map_or(0, |stats| {
        let prefix = format!("{}/", stats.owner);
        let local_for_owner = app
            .projects
            .iter()
            .filter(|project| {
                project
                    .remote_slug
                    .as_deref()
                    .is_some_and(|slug| slug.starts_with(&prefix))
            })
            .count();
        stats.total.saturating_sub(local_for_owner)
    });
    let github_lines = if columns.len() >= 5 {
        vec![
            ("PUBLIC", github.map_or(0, |stats| stats.public)),
            ("PRIVATE", github.map_or(0, |stats| stats.private)),
            ("CLOUD", cloud_only),
            ("ACTIVE", github.map_or(0, |stats| stats.active_30d)),
            ("STALE", github.map_or(0, |stats| stats.stale_1y_unarchived)),
            ("ARCHIVE", github.map_or(0, |stats| stats.archived)),
        ]
    } else {
        vec![
            ("PUBLIC", github.map_or(0, |stats| stats.public)),
            ("PRIVATE", github.map_or(0, |stats| stats.private)),
            ("CLOUD", cloud_only),
            ("STALE", github.map_or(0, |stats| stats.stale_1y_unarchived)),
            (
                "NO DESC",
                github.map_or(0, |stats| stats.public_missing_description),
            ),
            (
                "NO LIC",
                github.map_or(0, |stats| stats.public_missing_license),
            ),
        ]
    }
    .into_iter()
    .map(|(name, count)| compact_field(name, &count.to_string(), theme))
    .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(github_lines).block(panel(" GITHUB ", theme, true)),
        columns[2],
    );

    let stale = match github {
        Some(stats) if stats.stale_projects.is_empty() => vec![Line::from(Span::styled(
            " Nothing waiting for archive.",
            Style::default().fg(theme.good),
        ))],
        Some(stats) => stats
            .stale_projects
            .iter()
            .take(6)
            .map(|project| {
                let age = if project.age_days >= 730 {
                    format!("{}y", project.age_days / 365)
                } else {
                    format!("{}m", project.age_days / 30)
                };
                Line::from(vec![
                    Span::styled(
                        format!(" {:>3} ", age),
                        Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        if project.private { "◆ " } else { "○ " },
                        Style::default().fg(theme.faint),
                    ),
                    Span::styled(project.name.clone(), Style::default().fg(theme.text)),
                ])
            })
            .collect(),
        None if app.portfolio_loading => vec![Line::from(Span::styled(
            " Loading GitHub inventory…",
            Style::default().fg(theme.muted),
        ))],
        None => vec![Line::from(Span::styled(
            " GitHub inventory unavailable.",
            Style::default().fg(theme.muted),
        ))],
    };
    if columns.len() == 4 {
        frame.render_widget(
            Paragraph::new(stale).block(panel(" STALE · REVIEW ", theme, true)),
            columns[3],
        );
    } else if columns.len() >= 5 {
        let hygiene = [
            (
                "DESC",
                github.map_or("—".into(), |stats| {
                    format!(
                        "{}/{}",
                        stats
                            .public
                            .saturating_sub(stats.public_missing_description),
                        stats.public
                    )
                }),
            ),
            (
                "LICENSE",
                github.map_or("—".into(), |stats| {
                    format!(
                        "{}/{}",
                        stats.public.saturating_sub(stats.public_missing_license),
                        stats.public
                    )
                }),
            ),
            (
                "TOPICS",
                github.map_or("—".into(), |stats| {
                    format!(
                        "{}/{}",
                        stats.public.saturating_sub(stats.public_missing_topics),
                        stats.public
                    )
                }),
            ),
            (
                "HOME",
                github.map_or("—".into(), |stats| {
                    format!("{}/{}", stats.with_homepage, stats.total)
                }),
            ),
            (
                "RELEASE",
                github.map_or("—".into(), |stats| {
                    format!("{}/{}", stats.with_releases, stats.total)
                }),
            ),
        ]
        .into_iter()
        .map(|(name, value)| compact_field(name, &value, theme))
        .collect::<Vec<_>>();
        frame.render_widget(
            Paragraph::new(hygiene).block(panel(" PUBLIC HYGIENE ", theme, true)),
            columns[3],
        );
        frame.render_widget(
            Paragraph::new(stale).block(panel(" STALE · REVIEW ", theme, true)),
            columns[4],
        );
    }
}

fn draw_overview(frame: &mut Frame<'_>, project: &Project, theme: Theme, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Min(3),
    ])
    .split(area);

    let description = project
        .description
        .as_deref()
        .unwrap_or("No project description yet.");
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    &project.name,
                    Style::default().fg(theme.text).add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    match project.profile.source {
                        crate::model::ProfileSource::Declared => format!(
                            "  {} · {} · {}",
                            project.profile.kind.label(),
                            project.profile.lifecycle.label(),
                            project.profile.tier.label(),
                        ),
                        crate::model::ProfileSource::Inferred => format!(
                            "  {} · {} · inferred",
                            project.profile.kind.label(),
                            project.profile.lifecycle.label(),
                        ),
                    },
                    Style::default().fg(theme.faint),
                ),
            ]),
            Line::from(Span::styled(description, Style::default().fg(theme.muted))),
        ])
        .wrap(Wrap { trim: true })
        .block(panel(" PROJECT ", theme, true)),
        rows[0],
    );

    let gauge = LineGauge::default()
        .block(panel(
            format!(" EFFECTIVENESS · {} ", project.grade().to_uppercase()),
            theme,
            true,
        ))
        .filled_symbol("━")
        .unfilled_symbol("─")
        .filled_style(
            Style::default()
                .fg(theme.score(project.score.total))
                .add_modifier(Modifier::BOLD),
        )
        .unfilled_style(Style::default().fg(theme.faint))
        .style(Style::default().fg(theme.muted))
        .ratio(f64::from(project.score.total) / 100.0)
        .label(format!(
            "{} / 100 · {}% evidence",
            project.score.total, project.score.confidence
        ));
    frame.render_widget(gauge, rows[1]);

    let cards = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .spacing(1)
    .split(rows[2]);
    metric(
        frame,
        cards[0],
        "MOMENTUM",
        &project.git.commits_30d.to_string(),
        "commits · 30d",
        theme.secondary,
        theme,
    );
    metric(
        frame,
        cards[1],
        "DOCS",
        &format!("{}/6", project.docs.count()),
        "signals present",
        theme.accent,
        theme,
    );
    let (cairn_value, cairn_note) =
        project
            .cairn
            .as_ref()
            .map_or(("—".into(), "not configured".into()), |c| {
                (
                    format!("{}%", c.completion),
                    format!("{} open · {} active", c.open, c.active),
                )
            });
    metric(
        frame,
        cards[2],
        "CAIRN",
        &cairn_value,
        &cairn_note,
        theme.planning,
        theme,
    );
    let (deploy_value, deploy_note, deploy_color) = match project.site.healthy {
        Some(true) => (
            "UP",
            project
                .site
                .status_code
                .map(|s| format!("HTTP {s}"))
                .unwrap_or_default(),
            theme.good,
        ),
        Some(false) => (
            "DOWN",
            project
                .site
                .status_code
                .map(|s| format!("HTTP {s}"))
                .unwrap_or_default(),
            theme.bad,
        ),
        None if project.site.source => ("SITE", "awaiting check".into(), theme.warn),
        None => ("—", "no site detected".into(), theme.faint),
    };
    metric(
        frame,
        cards[3],
        "DEPLOY",
        deploy_value,
        &deploy_note,
        deploy_color,
        theme,
    );

    let signals = project
        .score
        .signals
        .iter()
        .map(|signal| {
            let (mark, color) = match signal.kind {
                SignalKind::Good => ("●", theme.good),
                SignalKind::Warn => ("◆", theme.warn),
                SignalKind::Bad => ("!", theme.bad),
                SignalKind::Info => ("•", theme.secondary),
            };
            Line::from(vec![
                Span::styled(
                    format!(" {mark} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(signal.label.clone(), Style::default().fg(theme.text)),
            ])
        })
        .collect::<Vec<_>>();
    if rows[3].height >= 7 && rows[3].width >= 72 {
        let bottom =
            Layout::horizontal([Constraint::Ratio(3, 5), Constraint::Ratio(2, 5)]).split(rows[3]);
        frame.render_widget(
            Paragraph::new(signals).block(panel(" SIGNALS ", theme, true)),
            bottom[0],
        );
        draw_score_shape(frame, project, theme, bottom[1]);
    } else {
        frame.render_widget(
            Paragraph::new(signals).block(panel(" SIGNALS ", theme, true)),
            rows[3],
        );
    }
}

fn draw_score_shape(frame: &mut Frame<'_>, project: &Project, theme: Theme, area: Rect) {
    let width = area.width.saturating_sub(14).clamp(4, 14) as usize;
    let axes = [
        ("SHAPE", project.score.local, 15),
        ("DOCS", project.score.documentation, 15),
        ("MOTION", project.score.momentum, 20),
        ("DELIVERY", project.score.delivery, 20),
        ("PLANNING", project.score.planning, 20),
        ("REACH", project.score.community, 10),
    ];
    let lines = axes
        .into_iter()
        .map(|(label, value, maximum)| {
            let percent = value.saturating_mul(100) / maximum;
            Line::from(vec![
                Span::styled(format!(" {label:<9}"), Style::default().fg(theme.faint)),
                Span::styled(
                    meter(value, maximum, width),
                    Style::default().fg(theme.score(percent)),
                ),
                Span::styled(format!(" {value:>2}"), Style::default().fg(theme.muted)),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(lines).block(panel(" SCORE SHAPE ", theme, true)),
        area,
    );
}

fn draw_git(frame: &mut Frame<'_>, project: &Project, theme: Theme, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(6),
        Constraint::Length(3),
        Constraint::Length(4),
        Constraint::Min(0),
    ])
    .split(area);
    frame.render_widget(
        Sparkline::default()
            .block(panel(" 12-WEEK COMMIT ACTIVITY ", theme, true))
            .data(&project.git.activity)
            .style(Style::default().fg(theme.secondary)),
        rows[0],
    );
    let metrics = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .spacing(1)
    .split(rows[1]);
    metric(
        frame,
        metrics[0],
        "COMMITS",
        &project.git.commits.to_string(),
        "all time",
        theme.accent,
        theme,
    );
    metric(
        frame,
        metrics[1],
        "PEOPLE",
        &project.git.contributors.to_string(),
        "contributors",
        theme.secondary,
        theme,
    );
    metric(
        frame,
        metrics[2],
        "BRANCHES",
        &project.git.branches.to_string(),
        &format!(
            "{} tags · {} files",
            project.git.tags, project.git.tracked_files
        ),
        theme.planning,
        theme,
    );
    metric(
        frame,
        metrics[3],
        "WORKTREE",
        &project.git.dirty.to_string(),
        "changed files",
        if project.git.dirty == 0 {
            theme.good
        } else {
            theme.warn
        },
        theme,
    );

    let sync = format!(
        " {}  branch {}     ↑ {} ahead     ↓ {} behind     +{} / −{} lines (30d)",
        if project.git.dirty == 0 {
            "clean"
        } else {
            "dirty"
        },
        project.git.branch,
        project.git.ahead,
        project.git.behind,
        project.git.additions_30d,
        project.git.deletions_30d,
    );
    let languages = if project.git.languages.is_empty() {
        "no source languages detected".into()
    } else {
        project
            .git
            .languages
            .iter()
            .map(|(name, files)| format!("{name} {files}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let last = format!(
        "{}  {}",
        project.git.last_commit_date.as_deref().unwrap_or("—"),
        project.git.last_subject.as_deref().unwrap_or("No commits")
    );
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(sync),
            Line::from(Span::styled(
                format!(" {languages}"),
                Style::default().fg(theme.faint),
            )),
            Line::from(vec![
                Span::styled(" LAST  ", Style::default().fg(theme.faint)),
                Span::styled(last, Style::default().fg(theme.text)),
            ]),
        ])
        .wrap(Wrap { trim: true })
        .style(Style::default().fg(theme.muted))
        .block(panel(" REPOSITORY ", theme, true)),
        rows[2],
    );
}

fn draw_delivery(frame: &mut Frame<'_>, project: &Project, theme: Theme, area: Rect) {
    let halves = Layout::vertical([
        Constraint::Length(7),
        Constraint::Length(6),
        Constraint::Min(0),
    ])
    .split(area);
    let top =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(halves[0]);
    let bottom =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(halves[1]);

    checklist(
        frame,
        top[0],
        " DOCUMENTATION ",
        &[
            ("README", project.docs.readme),
            ("license", project.docs.license),
            ("docs directory", project.docs.docs_dir),
            ("API docs", project.docs.api_docs),
            ("contributing guide", project.docs.contributing),
            ("changelog", project.docs.changelog),
        ],
        theme,
    );
    checklist(
        frame,
        top[1],
        " DELIVERY ",
        &[
            ("CI workflow", project.delivery.ci),
            ("test suite", project.delivery.tests),
            ("package manifest", project.delivery.package),
            ("release automation", project.delivery.release_automation),
            ("dependency updates", project.delivery.dependency_updates),
            ("site source", project.site.source),
        ],
        theme,
    );

    let site_lines = vec![
        field(
            "URL",
            project.site.url.as_deref().unwrap_or("not detected"),
            theme,
        ),
        field("Deployed", yes_no_unknown(project.site.deployed), theme),
        field("Health", yes_no_unknown(project.site.healthy), theme),
        field(
            "HTTP",
            &project
                .site
                .status_code
                .map(|s| s.to_string())
                .unwrap_or_else(|| "—".into()),
            theme,
        ),
        field(
            "Latency",
            &project
                .site
                .latency_ms
                .map(|ms| format!("{ms} ms"))
                .unwrap_or_else(|| "—".into()),
            theme,
        ),
    ];
    frame.render_widget(
        Paragraph::new(site_lines)
            .wrap(Wrap { trim: false })
            .block(panel(" SITE & DEPLOYMENT ", theme, true)),
        bottom[0],
    );

    let gh_lines = if let Some(gh) = &project.github {
        vec![
            field(
                "PRs",
                &format!(
                    "{} open · {} merged · {} closed",
                    gh.open_prs, gh.merged_prs, gh.closed_prs
                ),
                theme,
            ),
            field(
                "Community",
                &format!(
                    "{} issues · {} stars · {} forks",
                    gh.open_issues, gh.stars, gh.forks
                ),
                theme,
            ),
            field(
                "Reach · 14d",
                &format!(
                    "{} views · {} clones",
                    gh.unique_views_14d
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "—".into()),
                    gh.unique_clones_14d
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "—".into())
                ),
                theme,
            ),
            field(
                "Release",
                &format!(
                    "{} · {} downloads",
                    gh.latest_release.as_deref().unwrap_or("none"),
                    gh.release_downloads
                        .map(|value| value.to_string())
                        .unwrap_or_else(|| "—".into())
                ),
                theme,
            ),
            field(
                "Workflow",
                gh.latest_workflow
                    .as_deref()
                    .or(gh.ci_state.as_deref())
                    .unwrap_or("no checks"),
                theme,
            ),
        ]
    } else {
        vec![Line::from(Span::styled(
            " Remote data is loading or unavailable.",
            Style::default().fg(theme.muted),
        ))]
    };
    frame.render_widget(
        Paragraph::new(gh_lines)
            .wrap(Wrap { trim: false })
            .block(panel(" GITHUB ", theme, true)),
        bottom[1],
    );
}

fn draw_cairn(frame: &mut Frame<'_>, project: &Project, theme: Theme, area: Rect) {
    let Some(cairn) = &project.cairn else {
        let text = vec![
            Line::from(Span::styled(
                "No cairn.toml found",
                Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(Span::styled(
                "Run `cairn init` in this repository to add a roadmap.",
                Style::default().fg(theme.muted),
            )),
        ];
        frame.render_widget(
            Paragraph::new(text)
                .alignment(Alignment::Center)
                .block(panel(" CAIRN ", theme, true)),
            area,
        );
        return;
    };
    let rows = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(3),
        Constraint::Length(7),
        Constraint::Min(0),
    ])
    .split(area);
    frame.render_widget(
        LineGauge::default()
            .block(panel(" ROADMAP COMPLETION ", theme, true))
            .ratio(f64::from(cairn.completion) / 100.0)
            .filled_symbol("━")
            .unfilled_symbol("─")
            .filled_style(
                Style::default()
                    .fg(theme.planning)
                    .add_modifier(Modifier::BOLD),
            )
            .unfilled_style(Style::default().fg(theme.faint))
            .style(Style::default().fg(theme.muted))
            .label(format!(
                "{}% · {} of {} done",
                cairn.completion, cairn.done, cairn.total
            )),
        rows[0],
    );
    let cards = Layout::horizontal([
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
        Constraint::Ratio(1, 4),
    ])
    .spacing(1)
    .split(rows[1]);
    metric(
        frame,
        cards[0],
        "OPEN",
        &cairn.open.to_string(),
        "not started",
        theme.muted,
        theme,
    );
    metric(
        frame,
        cards[1],
        "ACTIVE",
        &cairn.active.to_string(),
        "in motion",
        theme.warn,
        theme,
    );
    metric(
        frame,
        cards[2],
        "BLOCKED",
        &cairn.blocked.to_string(),
        "needs attention",
        if cairn.blocked == 0 {
            theme.good
        } else {
            theme.bad
        },
        theme,
    );
    metric(
        frame,
        cards[3],
        "MILESTONES",
        &cairn.milestones.to_string(),
        &format!("{} dropped", cairn.dropped),
        theme.secondary,
        theme,
    );

    let columns =
        Layout::horizontal([Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)]).split(rows[2]);
    item_list(
        frame,
        columns[0],
        " IN PROGRESS ",
        &cairn.active_items,
        theme.warn,
        theme,
    );
    item_list(
        frame,
        columns[1],
        " UP NEXT ",
        &cairn.next_items,
        theme.secondary,
        theme,
    );
}

fn metric(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    value: &str,
    note: &str,
    color: ratatui::style::Color,
    theme: Theme,
) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                title.to_string(),
                Style::default()
                    .fg(theme.faint)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                value.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                note.to_string(),
                Style::default().fg(theme.muted),
            )),
        ])
        .alignment(Alignment::Center),
        area,
    );
}

fn checklist(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    entries: &[(&str, bool)],
    theme: Theme,
) {
    let lines = entries
        .iter()
        .map(|(label, present)| {
            Line::from(vec![
                Span::styled(
                    if *present { " ✓ " } else { " · " },
                    Style::default().fg(if *present { theme.good } else { theme.faint }),
                ),
                Span::styled(
                    (*label).to_string(),
                    Style::default().fg(if *present { theme.text } else { theme.muted }),
                ),
            ])
        })
        .collect::<Vec<_>>();
    frame.render_widget(Paragraph::new(lines).block(panel(title, theme, true)), area);
}

fn field(label: &str, value: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<10}"), Style::default().fg(theme.faint)),
        Span::styled(value.to_string(), Style::default().fg(theme.text)),
    ])
}

fn compact_field(label: &str, value: &str, theme: Theme) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!(" {label:<8}"), Style::default().fg(theme.faint)),
        Span::styled(format!("{value:>3}"), Style::default().fg(theme.text)),
    ])
}

fn item_list(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    items: &[String],
    color: ratatui::style::Color,
    theme: Theme,
) {
    let lines = if items.is_empty() {
        vec![Line::from(Span::styled(
            " Nothing here.",
            Style::default().fg(theme.faint),
        ))]
    } else {
        items
            .iter()
            .map(|item| {
                Line::from(vec![
                    Span::styled(" • ", Style::default().fg(color)),
                    Span::raw(item.clone()),
                ])
            })
            .collect()
    };
    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: true })
            .block(panel(title, theme, true)),
        area,
    );
}

fn yes_no_unknown(value: Option<bool>) -> &'static str {
    match value {
        Some(true) => "yes",
        Some(false) => "no",
        None => "unknown",
    }
}

fn panel<'a>(title: impl Into<Line<'a>>, theme: Theme, focused: bool) -> Block<'a> {
    Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(if focused { theme.border } else { theme.faint }))
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.muted)
                .add_modifier(Modifier::BOLD),
        )
}

fn human_age(days: u64) -> String {
    match days {
        0 => "now".into(),
        1..=13 => format!("{days}d"),
        14..=59 => format!("{}w", days / 7),
        60..=729 => format!("{}mo", days / 30),
        _ => format!("{}y", days / 365),
    }
}

fn draw_empty(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let area = Rect {
        height: area.height.min(8),
        ..area
    };
    if app.scanning {
        let lines = vec![
            Line::from(Span::styled(
                "Scanning repositories…",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(Span::styled(
                format!("Reading local project signals below {}", app.root.display()),
                Style::default().fg(theme.muted),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .block(panel(" DISCOVERING ", theme, true)),
            area,
        );
        return;
    }
    if !app.filter.is_empty() {
        let lines = vec![
            Line::from(Span::styled(
                "No matching projects",
                Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
            )),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Filter: ", Style::default().fg(theme.faint)),
                Span::styled(app.filter.clone(), Style::default().fg(theme.text)),
            ]),
            Line::from(Span::styled(
                "Press Esc to clear the filter.",
                Style::default().fg(theme.muted),
            )),
        ];
        frame.render_widget(
            Paragraph::new(lines)
                .alignment(Alignment::Center)
                .block(panel(" FILTER ", theme, true)),
            area,
        );
        return;
    }
    let lines = vec![
        Line::from(Span::styled(
            "No Git repositories found",
            Style::default().fg(theme.warn).add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            format!("Looked one level below {}", app.root.display()),
            Style::default().fg(theme.muted),
        )),
        Line::from(Span::styled(
            "Try: jerk ~/Code",
            Style::default().fg(theme.secondary),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel(" PROJECTS ", theme, true)),
        area,
    );
}

fn draw_footer(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    if app.searching {
        let prompt = Line::from(vec![
            Span::styled(
                " / ",
                Style::default()
                    .fg(theme.secondary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(app.filter.clone(), Style::default().fg(theme.text)),
            Span::styled("█", Style::default().fg(theme.accent)),
            Span::styled(
                "   enter apply · esc clear",
                Style::default().fg(theme.faint),
            ),
        ]);
        frame.render_widget(Paragraph::new(prompt), area);
        return;
    }
    let error = if app.tab == Tab::Insight {
        app.ai_error
            .as_deref()
            .or(app.remote_error.as_deref())
            .unwrap_or(&app.message)
    } else {
        app.remote_error.as_deref().unwrap_or(&app.message)
    };
    let left = format!(" {error}");
    let right = footer_controls(app.tab, area.width);
    let chunks = Layout::horizontal([
        Constraint::Min(10),
        Constraint::Length(right.chars().count() as u16),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(left).style(Style::default().fg(theme.faint)),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(right)
            .alignment(Alignment::Right)
            .style(Style::default().fg(theme.muted)),
        chunks[1],
    );
}

fn footer_controls(tab: Tab, width: u16) -> &'static str {
    if tab == Tab::Insight && width >= 92 {
        " a analyze  p payload  ↑↓ select  ←→ view  ? help  q quit "
    } else if width < 72 {
        " ? help  q quit "
    } else if width < 110 {
        " ↑↓ select  ←→ view  / find  r refresh  ? help "
    } else {
        " ↑↓ select  ←→ view  / filter  s sort  r/R refresh  o open  ? help  q quit "
    }
}

fn draw_ai_payload(frame: &mut Frame<'_>, app: &App, theme: Theme, area: Rect) {
    let width = area.width.saturating_sub(4).min(110);
    let height = area.height.saturating_sub(4).min(24);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);
    let payload = app.current_ai_payload().unwrap_or_else(|| "{}".to_string());
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                "This is the complete request snapshot. The API key and prompt are never shown here.",
                Style::default().fg(theme.muted),
            )),
            Line::raw(""),
            Line::from(Span::styled(payload, Style::default().fg(theme.text))),
        ])
        .wrap(Wrap { trim: true })
        .block(
            panel(" OUTBOUND AI PAYLOAD · METRICS ONLY ", theme, true)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_bottom(
                    Line::from(" p/esc close · a analyze · --ai-payload prints full JSON ")
                        .right_aligned(),
                ),
        ),
        popup,
    );
}

fn draw_help(frame: &mut Frame<'_>, theme: Theme, area: Rect) {
    let width = area.width.saturating_sub(4).min(76);
    let height = area.height.saturating_sub(2).min(20);
    let popup = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    frame.render_widget(Clear, popup);

    let key = |value: &'static str| {
        Span::styled(
            format!(" {value:<10}"),
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )
    };
    let note = |value: &'static str| Span::styled(value, Style::default().fg(theme.text));
    let lines = vec![
        Line::from(vec![key("j / ↓"), note("next project")]),
        Line::from(vec![key("k / ↑"), note("previous project")]),
        Line::from(vec![key("g / G"), note("first / last project")]),
        Line::from(vec![key("h / l"), note("previous / next view")]),
        Line::from(vec![key("1 … 6"), note("jump directly to a view")]),
        Line::from(vec![key("a"), note("analyze in the Insight view")]),
        Line::from(vec![key("p"), note("preview the outbound AI payload")]),
        Line::raw(""),
        Line::from(vec![
            key("/"),
            note("filter by name, description, or remote"),
        ]),
        Line::from(vec![key("s"), note("sort by name, attention, or recency")]),
        Line::from(vec![key("r / R"), note("rescan local / refresh remote")]),
        Line::from(vec![key("o / enter"), note("open the selected project")]),
        Line::raw(""),
        Line::from(vec![
            key("mouse"),
            note("click tabs, projects, and footer commands"),
        ]),
        Line::from(vec![key("wheel"), note("move through projects")]),
        Line::from(vec![key("? / esc"), note("close this guide")]),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(
            panel(" KEYBOARD ", theme, true)
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .title_bottom(Line::from(" mouse supported · keyboard complete ").right_aligned()),
        ),
        popup,
    );
}

fn draw_too_small(frame: &mut Frame<'_>, theme: Theme, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "JERK",
            Style::default()
                .fg(theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            "Give the dashboard a little more room",
            Style::default().fg(theme.text),
        )),
        Line::from(Span::styled(
            format!("{}×{} now · needs at least 72×16", area.width, area.height),
            Style::default().fg(theme.faint),
        )),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .alignment(Alignment::Center)
            .block(panel(" PROJECT PULSE ", theme, true)),
        area,
    );
}

fn score_mark(score: u16, theme: Theme) -> (&'static str, ratatui::style::Color) {
    match score {
        75..=100 => ("●", theme.good),
        50..=74 => ("◆", theme.warn),
        _ => ("!", theme.bad),
    }
}

fn meter(value: u16, maximum: u16, width: usize) -> String {
    let filled = if maximum == 0 {
        0
    } else {
        usize::from(value.min(maximum)) * width / usize::from(maximum)
    };
    format!("{}{}", "━".repeat(filled), "─".repeat(width - filled))
}

fn activity_strip(values: &[u64], width: usize) -> String {
    const LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
    let start = values.len().saturating_sub(width);
    let shown = &values[start..];
    let maximum = shown.iter().copied().max().unwrap_or_default();
    let mut out = " ".repeat(width.saturating_sub(shown.len()));
    for value in shown {
        let level = value
            .saturating_mul(7)
            .checked_div(maximum)
            .unwrap_or_default() as usize;
        out.push(LEVELS[level]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ages_are_compact() {
        assert_eq!(human_age(0), "now");
        assert_eq!(human_age(21), "3w");
        assert_eq!(human_age(400), "13mo");
    }

    #[test]
    fn meters_are_fixed_width_and_bounded() {
        assert_eq!(meter(5, 10, 8), "━━━━────");
        assert_eq!(meter(20, 10, 4), "━━━━");
        assert_eq!(meter(0, 0, 3), "───");
    }

    #[test]
    fn activity_strips_normalize_recent_values() {
        assert_eq!(activity_strip(&[0, 1, 2], 5), "  ▁▄█");
        assert_eq!(activity_strip(&[], 3), "   ");
    }

    #[test]
    fn every_tab_has_a_click_target() {
        let area = Rect::new(0, 2, 140, 2);
        for expected in Tab::ALL {
            assert!(
                (0..area.width).any(|column| tab_at(area, column) == Some(expected)),
                "{} tab has no click target",
                expected.label()
            );
        }
    }

    #[test]
    fn insight_footer_commands_are_clickable() {
        let width = 140;
        let controls = footer_controls(Tab::Insight, width);
        let start = width - controls.chars().count() as u16;
        let analyze = controls
            .find("a analyze")
            .unwrap_or_else(|| panic!("analyze control is missing"));
        let payload = controls
            .find("p payload")
            .unwrap_or_else(|| panic!("payload control is missing"));

        assert_eq!(
            footer_action_at(Tab::Insight, width, start + analyze as u16 + 1),
            Some(MouseAction::Analyze)
        );
        assert_eq!(
            footer_action_at(Tab::Insight, width, start + payload as u16 + 1),
            Some(MouseAction::Payload)
        );
        assert_eq!(footer_action_at(Tab::Insight, width, 0), None);
    }

    #[test]
    fn project_list_offset_keeps_selection_visible() {
        assert_eq!(project_list_offset(0, 20, 5), 0);
        assert_eq!(project_list_offset(7, 20, 5), 3);
        assert_eq!(project_list_offset(19, 20, 5), 15);
    }
}
