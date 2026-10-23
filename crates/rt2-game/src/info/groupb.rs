//! Company book pages added for the remaining original screens (docs/reference/screens.md G2,
//! G5, G13): the Overview tab of `compndtl.win`, the Territories tab data and the full-screen
//! company list `compnlst.win`. Read-only over the save, like the rest of `info`.
//!
//! Offsets: finance.md 1 (company record), 13.1..13.4 (chairman performance, salary, bonus,
//! standing in the territories), 14.2 (manager table), scenarios.md (track and speed counters).

use super::*;

/// Company record offsets used here (finance.md 1, scenarios.md "empresa").
const CO_SALARY: usize = 0x134;
const CO_MANAGER: usize = 0x13c;
const CO_CHAIR_SINCE: usize = 0x470;
const CO_BONUS: usize = 0x474;
const CO_BONUS_YEAR: usize = 0x46e;
const CO_PERF: usize = 0x46a;
const CO_TRAINS: usize = 0x27;
const CO_STATIONS: usize = 0x172;
const CO_TRACK_ALL: usize = 0x182;
const CO_TRACK_SINGLE: usize = 0x186;
const CO_TRACK_DOUBLE: usize = 0x18a;
const CO_LOADS_YEAR: usize = 0x192;
const CO_SPEED_N: usize = 0x196;
const CO_SPEED_SUM: usize = 0x19a;
const CO_LOADS_LIFE: usize = 0x19e;
const CO_GOODWILL: usize = 0x4229;
const CO_LOGO: usize = 0x2b;
/// Territory blocks of the company record: rights byte at `+0x479 + t * 0x1ed`, standing
/// (i16) one byte later (finance.md 13.4).
const CO_TERR: usize = 0x479;
const CO_TERR_STRIDE: usize = 0x1ed;
/// `manager_table` of the executable image (finance.md 14.2): salary i32 at `+0x11 * i`.
const MANAGER_TABLE: u32 = 0x0054_fdad;

/// One territory as the company book's Territories tab lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TerritoryRow {
    pub slot: u32,
    pub region: u8,
    pub name: String,
    /// Rights bits of the company: 0 trains, 1 track, 2 buildings, 3 stations.
    pub rights: u8,
    /// Standing of the company in the territory, -10000..10000 (finance.md 13.4).
    pub goodwill: i32,
}

impl Info<'_> {
    /// The logo / colour index of a company (`+0x2b`, 0..40).
    pub fn company_logo(&self, slot: u32) -> usize {
        rec(&self.save.companies, slot).map_or(0, |r| usize::from(r.u8(CO_LOGO)).min(40))
    }

    /// The territories with the company's rights and goodwill (empty for a map without any).
    pub fn territory_rows(&self, company: u32) -> Vec<TerritoryRow> {
        let co = rec(&self.save.companies, company);
        self.save
            .territories
            .ordered()
            .map(|r| {
                let t = Rec(&r.data);
                let base = CO_TERR + r.slot * CO_TERR_STRIDE;
                TerritoryRow {
                    slot: r.slot as u32,
                    region: t.u8(0x2a),
                    name: {
                        let n = t.text(4, 0x24);
                        if n.is_empty() { DASH.to_owned() } else { n }
                    },
                    rights: co.as_ref().map_or(0, |c| c.u8(base)),
                    goodwill: co.as_ref().map_or(0, |c| i32::from(c.i16(base + 1))),
                }
            })
            .collect()
    }

    /// Portrait index (`bigp00..40`, `brplyr00..40`) of the chairman of `company`: byte `+1` of
    /// the persona's table record (finance.md 14.4), the persona being the person's `+0x52f`.
    pub fn chairman_portrait(&self, company: u32) -> Option<usize> {
        let chair = rec(&self.save.companies, company)?.i32(CO_CHAIRMAN);
        let p = rec(
            &self.save.persons,
            u32::try_from(chair).ok().filter(|&c| c > 0)?,
        )?;
        let persona = u32::from(p.u8(0x52f)).min(40);
        Some(usize::from(self.data.image.u8(0x0055_1e88 + persona * 0x23 + 1)).min(40))
    }

    /// Salary of manager `i` from the executable's table (finance.md 14.2).
    fn manager_salary(&self, i: i32) -> i32 {
        self.data.image.i32(MANAGER_TABLE + 0x11 * i as u32)
    }

    /// Company book, Overview tab (`compndtl.win`, manual chapter 7): the chairman and the
    /// manager with their pay, the bonus, and a quick look at the operation.
    pub fn overview_page(&self, slot: u32) -> Page {
        let Some(r) = rec(&self.save.companies, slot) else {
            return Page {
                title: self.t(111, "Finances"),
                left: self.with_banner(vec![Block::Note(self.a("No such company."))]),
                ..Page::default()
            };
        };
        let d = self.detailed();
        let s = self.standing(&r);
        let co = &s.co;
        let cell = |v: String| if d { v } else { DASH.to_owned() };
        let chair = r.i32(CO_CHAIRMAN);
        let mgr = r.i32(CO_MANAGER);
        let chairman = vec![
            (
                self.t(112, "Chairman"),
                if chair > 0 && d {
                    self.person_name(chair as u32)
                } else {
                    DASH.to_owned()
                },
            ),
            (
                self.t(113, "Salary %1/year")
                    .replace("%1", &money(f64::from(r.i32(CO_SALARY)))),
                String::new(),
            ),
            (self.a("Performance"), cell(r.i32(CO_PERF).to_string())),
            (
                self.a("Last bonus"),
                cell(if r.i32(CO_BONUS) > 0 {
                    format!(
                        "{} ({})",
                        money(f64::from(r.i32(CO_BONUS))),
                        r.i16(CO_BONUS_YEAR)
                    )
                } else {
                    DASH.to_owned()
                }),
            ),
            (
                self.a("Chairman since"),
                cell(r.i16(CO_CHAIR_SINCE).to_string()),
            ),
        ];
        let mut manager = Vec::new();
        if d && (0..40).contains(&mgr) {
            manager.push((self.t(116, "Manager"), self.t(1456 + 2 * mgr as u32, "")));
            manager.push((
                self.t(113, "Salary %1/year")
                    .replace("%1", &money(f64::from(self.manager_salary(mgr)))),
                String::new(),
            ));
        } else {
            manager.push((self.t(116, "Manager"), DASH.to_owned()));
        }
        let mut left = vec![Block::Facts(chairman), Block::Facts(manager)];
        if d && (0..40).contains(&mgr) {
            left.push(Block::Note(self.t(1457 + 2 * mgr as u32, "")));
        }
        let year = |i: usize| co.cats[..i].iter().sum::<f64>();
        let revenue = year(5);
        let profit = co.cats.iter().sum::<f64>();
        let speed = r.i32(CO_SPEED_SUM) / r.i32(CO_SPEED_N).max(1);
        let ops = vec![
            (self.t(984, "Revenue"), cell(money(revenue))),
            (self.t(36, "Profit"), cell(money(profit))),
            (self.t(31, "Cash"), cell(money(co.acct[0]))),
            (
                self.t(979, "Debt"),
                cell(money(-co.acct[finance::ACCT_DEBT])),
            ),
            (
                self.t(1011, "Credit Rating"),
                s.price.map_or(DASH.into(), |_| self.grade(s.rating)),
            ),
            (
                self.t(1009, "Dividend Per Share"),
                cell(format!("${:.2}", co.dividend)),
            ),
            (
                self.t(1005, "Share Price"),
                s.price.map_or(DASH.into(), |p| money(f64::from(p))),
            ),
            (self.a("Founded"), cell(co.founded.to_string())),
        ];
        let network = vec![
            (self.t(978, "Trains"), cell(r.i32(CO_TRAINS).to_string())),
            (self.a("Stations"), cell(r.i32(CO_STATIONS).to_string())),
            (
                self.a("Track (single / double cells)"),
                cell(format!(
                    "{} / {}",
                    int(i64::from(r.i32(CO_TRACK_SINGLE))),
                    int(i64::from(r.i32(CO_TRACK_DOUBLE)))
                )),
            ),
            (
                self.a("Track cells laid"),
                cell(int(i64::from(r.i32(CO_TRACK_ALL)))),
            ),
            (
                self.a("Average speed"),
                cell(self.af("%1 mph", &[speed.to_string()])),
            ),
            (
                self.a("Loads hauled (year / lifetime)"),
                cell(format!(
                    "{} / {}",
                    int(i64::from(r.i32(CO_LOADS_YEAR))),
                    int(i64::from(r.i32(CO_LOADS_LIFE)))
                )),
            ),
            (self.a("Goodwill"), cell(format!("{}", r.i32(CO_GOODWILL)))),
        ];
        Page {
            title: self.company_name(slot),
            subtitle: format!(
                "{}: {}",
                self.t(112, "Chairman"),
                if chair > 0 && d {
                    self.person_name(chair as u32)
                } else {
                    DASH.to_owned()
                }
            ),
            left: self.with_banner(left),
            right: vec![Block::Facts(ops), Block::Facts(network)],
        }
    }

    /// Territories tab, the table beside the map views (`compndtl.win`).
    pub fn territories_page(&self, company: u32) -> Page {
        let rows = self.territory_rows(company);
        let mut t = Table::new(&[
            (&self.a("Territory"), 3.0, false),
            (&self.a("Run trains"), 1.2, true),
            (&self.a("Lay track"), 1.2, true),
            (&self.a("Buildings"), 1.2, true),
            (&self.a("Stations"), 1.2, true),
            (&self.a("Goodwill"), 1.2, true),
        ]);
        let yn = |on: bool| if on { self.a("yes") } else { self.a("no") };
        for r in &rows {
            t.push(
                r.slot,
                vec![
                    r.name.clone(),
                    yn(r.rights & 1 != 0),
                    yn(r.rights & 2 != 0),
                    yn(r.rights & 4 != 0),
                    yn(r.rights & 8 != 0),
                    format!("{}", r.goodwill / 100),
                ],
            );
        }
        Page {
            title: self.company_name(company),
            subtitle: self.t(2369, "Territories"),
            left: vec![],
            right: vec![if rows.is_empty() {
                Block::Note(self.a("This map has no political divisions."))
            } else {
                Block::Table {
                    title: self.t(2369, "Territories"),
                    table: t,
                }
            }],
        }
    }

    /// The full-screen company list (`compnlst.win`): one row per active company; the row id
    /// is the pool slot.
    pub fn companies_list(&self) -> ListPage {
        let mut t = Table::new(&[
            (&self.t(178, "Company"), 3.0, false),
            (&self.t(112, "Chairman"), 2.6, false),
            (&self.t(31, "Cash"), 1.6, true),
            (&self.t(984, "Revenue"), 1.6, true),
            (&self.t(36, "Profit"), 1.6, true),
            (&self.t(43, "Price"), 1.0, true),
            (&self.a("Rating"), 0.9, true),
            (&self.a("Founded"), 0.9, true),
        ]);
        let d = self.detailed();
        for r in self.save.companies.ordered() {
            let rr = Rec(&r.data);
            if d && rr.i32(CO_ACTIVE) == 0 {
                continue;
            }
            let s = self.standing(&rr);
            let mark = if self.is_human_company(&rr) { " *" } else { "" };
            let slot = r.slot as u32;
            let cell = |v: String| if d { v } else { DASH.to_owned() };
            t.push(
                slot,
                vec![
                    format!("{}{mark}", self.company_name(slot)),
                    self.chairman_name(slot),
                    cell(money(s.co.acct[0])),
                    cell(money(s.co.cats[..5].iter().sum::<f64>())),
                    cell(money(s.co.cats.iter().sum::<f64>())),
                    s.price.map_or(DASH.into(), |p| money(f64::from(p))),
                    s.price.map_or(DASH.into(), |_| self.grade(s.rating)),
                    cell(s.co.founded.to_string()),
                ],
            );
        }
        ListPage {
            title: self.a("Companies"),
            table: t,
        }
    }
}

// ---- train detail: instrument panel (screens.md E2, E4) ---------------------------------------

/// The numbers of the instrument panel of `traindtl.win` (trains.md 1.1): sand `+0xcf`, oil
/// `+0xd3`, water `+0xd7` (f32, 0..100), throttle `+0xdb` (85 = normal, above is the red zone),
/// the train class `+0xa7` (0 Halted, 1 Slow, 2 Normal, 3 Express) and the engine kind.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TrainPanel {
    pub sand: f32,
    pub oil: f32,
    pub water: f32,
    pub throttle: f32,
    pub class: u8,
    /// 0 steam, 1 diesel, 2 electric (`engine_table +0x28`); `None` when unknown.
    pub kind: Option<u8>,
    pub age: i32,
    pub tons: i32,
    pub broken: bool,
}

impl Info<'_> {
    pub fn train_panel(&self, slot: u32) -> Option<TrainPanel> {
        let t = rec(&self.save.trains, slot)?;
        let kind = self.cars(&t).iter().find_map(|c| {
            let v = self.data.vehicles.get(usize::from(c.u8(8)))?;
            (v.species == 0 && v.engine >= 0)
                .then(|| self.data.engines.get(v.engine as usize).map(|e| e.kind))
                .flatten()
        });
        Some(TrainPanel {
            sand: t.f32(0xcf),
            oil: t.f32(0xd3),
            water: t.f32(0xd7),
            throttle: t.f32(0xdb),
            class: t.u8(0xa7),
            kind,
            age: (self.year - i32::from(t.u16(0xbd))).max(0),
            tons: t.i32(0xaf),
            broken: t.u8(0xed) != 0,
        })
    }

    /// Cargo types a car can be bought for (vehicle types of species car with a cargo,
    /// without the caboose and the restaurant car), sorted by cargo id, with their names.
    pub fn car_cargo_choices(&self) -> Vec<(u8, String)> {
        let mut ids: Vec<u8> = self
            .data
            .vehicles
            .iter()
            .filter(|v| v.species == 2 && v.cargo < 0x2c)
            .map(|v| v.cargo)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids.into_iter()
            .map(|c| (c, self.cargo_name(c)))
            // The LNG table pads its last entries with placeholders ("Cargo 34").
            .filter(|(c, n)| {
                *n != self.af("cargo %1", &[c.to_string()])
                    && !n.is_empty()
                    && !n.ends_with(|ch: char| ch.is_ascii_digit())
            })
            .collect()
    }

    pub fn cargo_label(&self, c: u8) -> String {
        self.cargo_name(c)
    }

    /// Stop `k` of a train: station name, sell mode (flags bits 4..5).
    pub fn stop_sell_mode(&self, train: u32, k: usize) -> u8 {
        self.save
            .train_extras
            .iter()
            .find(|e| e.slot == train as usize)
            .and_then(|e| e.cars_raw.get(k * 14 + 3))
            .map_or(0, |f| (f >> 4) & 3)
    }
}

// ---- stock ticker (screens.md C8) -------------------------------------------------------------

impl Info<'_> {
    /// The stock ticker's items, one per active company with shares: name, share price and the
    /// change since the end of last month (`+0x4282`, finance.md 1).
    pub fn ticker_items(&self) -> Vec<(String, i32, f64)> {
        let mut v = Vec::new();
        for r in self.save.companies.ordered() {
            let rr = Rec(&r.data);
            if self.detailed() && rr.i32(CO_ACTIVE) == 0 {
                continue;
            }
            let s = self.standing(&rr);
            if let Some(p) = s.price {
                v.push((
                    self.company_name(r.slot as u32),
                    p,
                    f64::from(p) - f64::from(s.co.month_end_price),
                ));
            }
        }
        v
    }
}
