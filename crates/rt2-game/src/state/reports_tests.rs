//! The Reports screen (docs/ui.md "Relatórios e gráficos") driven through the real input path:
//! synthetic cursor moves and presses on the laid-out UI and key events. They need the install
//! (`RT2_DIR` or the default folder) and its `games/Example save game.GAM`; skipped without it.

use std::path::PathBuf;
use std::sync::Arc;

use rt2_render::GameAssets;
use rt2_ui::{Kind, UiScale};
use winit::keyboard::KeyCode;

use super::dialogs::Dialog;
use super::info_ui::InfoScreen;
use super::reports_ui::*;
use super::*;
use crate::reports::{self, METRICS, Metric};

fn state(viewport: (u32, u32)) -> Option<AppState> {
    let install = std::env::var_os("RT2_DIR")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Games/railroad-tycoon-2"))
        })
        .filter(|p| p.join("RT2_PLAT.EXE").is_file())?;
    let assets = Arc::new(GameAssets::open(&install).ok()?);
    let art = crate::artwork::load(&assets);
    let init = Init {
        install: install.clone(),
        assets,
        backdrop: art.backdrop,
        icons: art.icons,
        dialogs: art.dialogs,
        config: Config::default(),
        scale: UiScale::from_percent(100),
        scale_pinned: true,
        viewport,
    };
    let mut s = AppState::new(init);
    s.set_saves_dir(None);
    s.start(&install.join("games/Example save game.GAM")).ok()?;
    s.update(0.0);
    while s.has_dialog() {
        s.close_dialog();
    }
    s.update(0.0);
    Some(s)
}

fn game(s: &AppState) -> &GameState {
    match &s.screen {
        Screen::Game(g) => g,
        Screen::Menu(_) => panic!("no game"),
    }
}

fn dlg(s: &AppState) -> ReportsDlg {
    match game(s).dialogs.last() {
        Some(Dialog::Reports(r)) => r.clone(),
        d => panic!("expected the Reports window, got {d:?}"),
    }
}

/// Physical middle of the first enabled control with `action`.
fn center_of(s: &AppState, action: u32) -> Option<(f32, f32)> {
    let k = s.scale.factor();
    s.ui.draw_order().into_iter().find_map(|(_, id)| {
        let hit = match &s.ui.node(id).kind {
            Kind::Button {
                action: a, enabled, ..
            }
            | Kind::Toggle {
                action: a, enabled, ..
            }
            | Kind::Hotspot {
                action: a, enabled, ..
            } => *a == action && *enabled,
            Kind::Icon { action: a, .. } | Kind::Cells { action: a, .. } => *a == action,
            _ => false,
        };
        hit.then(|| {
            let r = s.ui.rect(id);
            ((r.x + r.w / 2.0) * k, (r.y + r.h / 2.0) * k)
        })
    })
}

fn press_at(s: &mut AppState, p: (f32, f32)) {
    s.cursor_moved(p.0, p.1);
    s.mouse_button(true, true);
    s.mouse_button(true, false);
    s.update(0.0);
}

fn click(s: &mut AppState, action: u32) {
    let p = center_of(s, action).unwrap_or_else(|| panic!("no enabled control {action}"));
    press_at(s, p);
}

fn has_control(s: &AppState, action: u32) -> bool {
    center_of(s, action).is_some()
}

fn labels(s: &AppState) -> Vec<String> {
    s.ui.draw_order()
        .into_iter()
        .filter_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::Label { text, .. } => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The colours of the chart raster.
fn chart(s: &AppState) -> Vec<rt2_ui::Color> {
    s.ui.draw_order()
        .into_iter()
        .find_map(|(_, id)| match &s.ui.node(id).kind {
            Kind::Cells {
                action: 14090,
                colors,
                ..
            } => Some(colors.as_ref().clone()),
            _ => None,
        })
        .expect("the chart")
}

fn camera(s: &AppState) -> (f32, f32) {
    let t = game(s).camera.target;
    (t.x, t.y)
}

/// A scratch folder under `~/.cache/rrt2/`, emptied first.
fn scratch(name: &str) -> PathBuf {
    let dir = PathBuf::from(std::env::var_os("HOME").unwrap())
        .join(".cache/rrt2/tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

#[test]
fn the_company_book_opens_the_reports_and_the_tabs_and_figures_switch_by_clicks() {
    let Some(mut s) = state((1920, 1080)) else {
        eprintln!("RT2_DIR is missing; skipping");
        return;
    };
    s.open_info(InfoScreen::Finance(0));
    assert!(!matches!(game(&s).dialogs.last(), Some(Dialog::Reports(_))));
    click(&mut s, A_REP_OPEN);
    assert_eq!(dlg(&s).tab, 0);
    assert_eq!(dlg(&s).metric, 0);
    let revenue = chart(&s);
    assert!(
        revenue.iter().any(|c| reports::PALETTE.contains(c)),
        "the chart draws the series"
    );
    // The player's company has archived years: more than one yearly point plus the year to date.
    let (companies, now) = {
        let d = s.reports_data_for_test().unwrap();
        (d.0, d.1)
    };
    let series = reports::build_series(Metric::Revenue, &companies, &game(&s).reports, now);
    assert!(
        series.iter().any(|x| x.points.len() >= 3),
        "archive plus the year to date: {:?}",
        series.iter().map(|x| x.points.len()).collect::<Vec<_>>()
    );

    // Another figure by its button: the raster changes and the label of the button is in play.
    click(&mut s, 14020 + 6);
    assert_eq!(dlg(&s).metric, 6);
    assert_eq!(METRICS[6], Metric::Price);
    assert_ne!(chart(&s), revenue);
    // Hiding the first company in the legend redraws without it; showing it again restores.
    let with_all = chart(&s);
    click(&mut s, 14040);
    assert_eq!(dlg(&s).hidden & 1, 1);
    assert_ne!(chart(&s), with_all);
    click(&mut s, 14040);
    assert_eq!(dlg(&s).hidden, 0);
    assert_eq!(chart(&s), with_all);

    // A press on the chart sets the cursor, a key moves it to the next point in time.
    let before = dlg(&s).cursor;
    assert!(before.is_none());
    click(&mut s, 14090);
    let c0 = dlg(&s).cursor.expect("the press set the cursor");
    assert!((0.0..=1.0).contains(&c0));
    s.key(KeyCode::ArrowRight, true, false, None);
    s.key(KeyCode::ArrowRight, false, false, None);
    assert_ne!(dlg(&s).cursor, Some(c0));

    // The income tab lists the categories with LNG labels and the years of the archive.
    click(&mut s, 14010 + 1);
    assert_eq!(dlg(&s).tab, 1);
    let l = labels(&s);
    for want in [
        "Freight",
        "Revenue",
        "Expenses",
        "Profit",
        "Lifetime",
        "This year",
    ] {
        assert!(l.iter().any(|x| x == want), "{want} in {l:?}");
    }
    let last_year = (s.date_ym().0 - 1).to_string();
    assert!(l.contains(&last_year), "the last archived year {last_year}");
    // The arrows cycle the company: the dialog's company and the year header change with it.
    assert!(companies.len() >= 2);
    let shown_first = dlg(&s).company;
    click(&mut s, 14092);
    let next = dlg(&s).company;
    assert_ne!(next, shown_first);
    assert!(companies.iter().any(|c| c.slot == next));
    s.key(KeyCode::ArrowLeft, true, false, None);
    s.key(KeyCode::ArrowLeft, false, false, None);
    assert_eq!(dlg(&s).company, shown_first);

    // Back to the charts by the key 1, the rankings by the key 3; Escape closes.
    s.key(KeyCode::Digit1, true, false, None);
    s.key(KeyCode::Digit1, false, false, None);
    assert_eq!(dlg(&s).tab, 0);
    s.key(KeyCode::Digit3, true, false, None);
    s.key(KeyCode::Digit3, false, false, None);
    assert_eq!(dlg(&s).tab, 2);
    s.key(KeyCode::Escape, true, false, None);
    assert!(!s.has_dialog());
}

#[test]
fn a_ranking_row_centers_the_camera_and_a_second_click_opens_the_train_window() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    s.open_reports(0);
    click(&mut s, 14010 + 2);
    assert_eq!(dlg(&s).tab, 2);
    let trains = s.train_rows(0);
    assert!(trains.len() >= 2, "the Example save has trains");
    // Best first by this year's profit.
    assert!(trains.windows(2).all(|w| w[0].year >= w[1].year));
    let first = &trains[0];
    let tile = first.tile.expect("a train has a place");
    let before = camera(&s);
    click(&mut s, 14110);
    assert_eq!(dlg(&s).sel, Some(0));
    assert_eq!(camera(&s), (tile.0 as f32 + 0.5, tile.1 as f32 + 0.5));
    assert!(before != camera(&s) || tile == (before.0 as i32, before.1 as i32));
    // The row of another train moves the camera there.
    click(&mut s, 14111);
    assert_eq!(dlg(&s).sel, Some(1));
    let t1 = trains[1].tile.unwrap();
    assert_eq!(camera(&s), (t1.0 as f32 + 0.5, t1.1 as f32 + 0.5));
    // A second click on the selected row opens its window and closes the report.
    click(&mut s, 14111);
    assert!(!matches!(game(&s).dialogs.last(), Some(Dialog::Reports(_))));
    let slot = trains[1].slot;
    let opened = matches!(
        game(&s).dialogs.last(),
        Some(Dialog::TrainEdit { train, .. }) if *train == slot as i32
    ) || s.info_screen() == Some(InfoScreen::TrainDetail(slot));
    assert!(opened, "the train's own window is open");

    // Lifetime ranking sorts by the accumulator since purchase.
    s.close_dialog();
    s.close_info();
    s.open_reports(0);
    click(&mut s, 14010 + 2);
    click(&mut s, 14100 + 2);
    let life = s.train_rows(2);
    assert!(life.windows(2).all(|w| w[0].life >= w[1].life));
    // The keyboard: Down selects the next row, Enter opens it.
    s.key(KeyCode::ArrowDown, true, false, None);
    s.key(KeyCode::ArrowDown, false, false, None);
    assert_eq!(dlg(&s).sel, Some(0));
    s.key(KeyCode::ArrowDown, true, false, None);
    s.key(KeyCode::ArrowDown, false, false, None);
    assert_eq!(dlg(&s).sel, Some(1));
    s.key(KeyCode::Enter, true, false, None);
    s.key(KeyCode::Enter, false, false, None);
    assert!(!matches!(game(&s).dialogs.last(), Some(Dialog::Reports(_))));
}

#[test]
fn the_station_ranking_opens_the_station_window_from_its_button() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    s.open_reports(0);
    click(&mut s, 14010 + 2);
    click(&mut s, 14100 + 3);
    let stations = s.station_rows();
    assert!(stations.len() >= 2);
    assert!(stations.windows(2).all(|w| w[0].moved >= w[1].moved));
    // Open is disabled until a row is selected.
    assert!(!has_control(&s, 14160));
    click(&mut s, 14110);
    let st = &stations[0];
    assert_eq!(camera(&s), (st.tile.0 as f32 + 0.5, st.tile.1 as f32 + 0.5));
    click(&mut s, 14160);
    assert!(!matches!(game(&s).dialogs.last(), Some(Dialog::Reports(_))));
    let opened = matches!(
        game(&s).dialogs.last(),
        Some(Dialog::Station { station }) if *station == st.slot as i32
    ) || s.info_screen() == Some(InfoScreen::StationDetail(st.slot));
    assert!(opened, "the station's window is open");
}

#[test]
fn a_small_window_pages_the_ranking_with_the_buttons_and_the_keys() {
    let Some(mut s) = state((800, 600)) else {
        return;
    };
    s.open_reports(0);
    click(&mut s, 14010 + 2);
    let n = s.train_rows(0).len();
    let visible = (0..40u32).filter(|i| has_control(&s, 14110 + i)).count();
    assert!(visible >= 1);
    if n > visible {
        assert!(!has_control(&s, 14161), "Page up is disabled at the top");
        click(&mut s, 14162);
        assert!(dlg(&s).scroll > 0);
        assert!(has_control(&s, 14161));
        s.key(KeyCode::PageUp, true, false, None);
        s.key(KeyCode::PageUp, false, false, None);
        assert_eq!(dlg(&s).scroll, 0);
        s.wheel(-1.0);
        assert!(dlg(&s).scroll > 0, "the wheel scrolls the table");
    }
}

#[test]
fn the_export_writes_the_current_report_to_csv_and_tells_the_path() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    let dir = scratch("reports-export");
    s.set_reports_dir(Some(dir.clone()));
    s.open_reports(0);

    // The chart of the revenue.
    click(&mut s, 14163);
    let n = dlg(&s).notice.expect("a notice with the path");
    assert!(n.contains(dir.to_str().unwrap()), "{n}");
    assert!(n.contains("chart-revenue"), "{n}");
    let (y, m) = s.date_ym();
    let file = dir.join(format!("rrt2-chart-revenue-{y}-{m:02}.csv"));
    let csv = std::fs::read_to_string(&file).unwrap();
    assert!(csv.starts_with("year,month,company,metric,value,year_to_date\n"));
    let rows: Vec<&str> = csv.lines().skip(1).collect();
    assert!(rows.len() >= 3);
    assert!(rows.iter().any(|r| r.ends_with(",1")), "the year to date");
    assert!(
        labels(&s).iter().any(|l| l.contains("chart-revenue")),
        "the notice is on screen"
    );

    // The share price, with a company hidden: its rows are not exported.
    click(&mut s, 14020 + 6);
    let names: Vec<String> = s
        .reports_data_for_test()
        .unwrap()
        .0
        .iter()
        .map(|c| c.name.clone())
        .collect();
    click(&mut s, 14040);
    click(&mut s, 14163);
    let csv = std::fs::read_to_string(dir.join(format!("rrt2-chart-share-price-{y}-{m:02}.csv")))
        .unwrap();
    assert!(csv.contains("Share Price"));
    assert!(
        !csv.contains(&names[0]) || names.iter().skip(1).any(|n| n == &names[0]),
        "the hidden company is left out"
    );

    // The income statement of the company on screen.
    click(&mut s, 14010 + 1);
    click(&mut s, 14163);
    let company = s
        .reports_data_for_test()
        .unwrap()
        .0
        .into_iter()
        .find(|c| c.human)
        .or_else(|| s.reports_data_for_test().unwrap().0.into_iter().next())
        .unwrap();
    let entry = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(Result::ok)
        .find(|e| e.file_name().to_string_lossy().starts_with("rrt2-income-"))
        .expect("the income file");
    let csv = std::fs::read_to_string(entry.path()).unwrap();
    let head: Vec<&str> = csv.lines().next().unwrap().split(',').collect();
    assert_eq!(
        &head[..4],
        ["company", "category", "lifetime", "year_to_date"]
    );
    assert_eq!(head[4], (y - 1).to_string());
    let profit = csv
        .lines()
        .find(|l| l.contains(",Profit,"))
        .expect("the profit line");
    // The profit of the first archived year is the sum of the 13 categories.
    let want: f64 = company.years[0].cats.iter().sum();
    let got: f64 = profit.split(',').nth(4).unwrap().parse().unwrap();
    assert!((got - want).abs() < 1.0, "{got} vs {want}");

    // The trains ranking.
    click(&mut s, 14010 + 2);
    click(&mut s, 14100 + 2);
    click(&mut s, 14163);
    let csv =
        std::fs::read_to_string(dir.join(format!("rrt2-trains-lifetime-{y}-{m:02}.csv"))).unwrap();
    assert!(csv.starts_with("rank,train,company,engine,this_year,last_year,lifetime\n"));
    let trains = s.train_rows(2);
    assert_eq!(csv.lines().count(), trains.len() + 1);
    let first: Vec<&str> = csv.lines().nth(1).unwrap().rsplitn(2, ',').collect();
    assert_eq!(
        first[0],
        trains[0].life.map_or_else(String::new, |v| v.to_string())
    );
    // And the stations.
    click(&mut s, 14100 + 3);
    click(&mut s, 14163);
    let csv = std::fs::read_to_string(dir.join(format!("rrt2-stations-{y}-{m:02}.csv"))).unwrap();
    assert!(csv.starts_with("rank,station,owner,lots_moved,lots_generated\n"));
    assert_eq!(csv.lines().count(), s.station_rows().len() + 1);

    // An unwritable folder is reported, not hidden.
    let blocker = dir.join("rrt2-stations-blocker");
    std::fs::write(&blocker, "x").unwrap();
    s.set_reports_dir(Some(blocker));
    click(&mut s, 14163);
    assert!(dlg(&s).notice.unwrap().starts_with("Cannot export"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_balances_are_sampled_every_game_month_while_the_days_pass() {
    let Some(mut s) = state((1920, 1080)) else {
        return;
    };
    let n0 = game(&s).reports.len();
    assert_eq!(n0, 1, "the start is sampled");
    s.advance_days(100);
    let n1 = game(&s).reports.len();
    assert!(n1 >= n0 + 3, "{n0} -> {n1}: about one row per 28 days");
    // The cash line has those samples plus the live value.
    let (companies, now) = s.reports_data_for_test().unwrap();
    let series = reports::build_series(Metric::Cash, &companies, &game(&s).reports, now);
    let mine = series.iter().find(|x| x.human).expect("the player's line");
    assert!(mine.points.len() >= n1);
    assert!(mine.points.windows(2).all(|w| w[0].t < w[1].t));
}
