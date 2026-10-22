//! Route planning of the AI (ai.md 5.1 to 5.3): `ai_town_value` (`0x4313f0`),
//! `ai_extend_network` (`0x435320`) and `ai_plan_first_line` (`0x435c20`).

use super::{Ai, CO_NET_STATIONS, CO_SATISFIED, CO_STATIONS, Trait};
use crate::fp::ftol;
use crate::trainstep::P;

impl Ai<'_, '_> {
    /// `ai_town_value(town, company)` (`0x4313f0`, ai.md 5.1) [V with company 0].
    pub fn town_value(&self, town: i32, company: i32) -> i32 {
        if self.ts.u8(P::Place, town, 0x10) != 0 {
            return 0;
        }
        let a = f64::from(self.ts.f32(P::Place, town, 0x12));
        let b = f64::from(self.ts.f32(P::Place, town, 0x3a));
        let mut v = ftol((a - b) * f64::from(60000.0f32));
        let class = self.ts.u8(P::Place, town, 0x48);
        if class >= 8 {
            v = v.wrapping_mul(2);
        } else if class >= 4 {
            v = ftol(f64::from(v) * 1.7);
        }
        if company == 0 {
            return v;
        }
        let cx = i32::from(self.ts.i16(P::Place, town, 4));
        let cy = i32::from(self.ts.i16(P::Place, town, 6));
        let terr = usize::from(self.ts.t8(cx, cy, 9));
        if self.ts.u8(P::Company, company, 0x479 + terr * 0x1ed) & 3 != 3 {
            return 0;
        }
        let n = self.town_station_count(town);
        let mut k = n;
        while k >= 1 {
            let st = self.town_station(town, k);
            if self.station_usable(st, company) {
                return 0;
            }
            k -= 1;
        }
        if n != 0 {
            v /= 2 * n + 1;
        }
        let bonus = self.ts.i32(P::Place, town, 0x36).wrapping_add(500_000);
        ftol(f64::from(bonus) * f64::from(v) * 2e-6)
    }

    /// `company_network_station(c, k)` (`0x43bbc0`): the `k`-th (1-based) station of the main
    /// network (`station+0x57 == company+0x17a`), -1 if none.
    pub fn network_station(&self, k: i32) -> i32 {
        if self.co_i32(super::CO_ACTIVE) == 0 {
            return 0;
        }
        let net = self.co_u32(super::CO_NETWORK);
        let mut k = k;
        for st in self.ids(P::Station) {
            if u32::from(self.ts.u16(P::Station, st, 0x57)) == net {
                k -= 1;
                if k == 0 {
                    return st;
                }
            }
        }
        -1
    }

    /// `ai_extend_network` (`0x435320`, ai.md 5.2).
    pub fn extend_network(&mut self) -> bool {
        self.ts.log.ran("ai_extend_network");
        let company = self.company;
        let mut cash = ftol(self.cash());
        let chair = self.chairman();
        if chair != 0 {
            let x = self.persona(chair, Trait::IssueBonds) + 200;
            cash = ftol(f64::from(x) * f64::from(cash) * (1.0 / 300.0));
        }
        if cash < 400_000 {
            return false;
        }
        let n_st = self.co_i32(CO_STATIONS);
        let te = self.track_expansion();
        match te {
            0 if n_st > 15 => return false,
            1 if n_st > 20 => return false,
            2 if n_st > 24 => return false,
            _ => {}
        }
        let cash_d = f64::from(cash);
        let mut result = [false; 10];
        let mut chosen = [0i32; 10];
        let mut score = [0i32; 10];
        let mut cost = [0i32; 10];
        let mut viable = 0;
        for p in 0..10 {
            result[p] = false;
            if viable >= 2 {
                continue;
            }
            let mut best = 0i32;
            let mut city = 0i32;
            let mut s_best = 0i32;
            for town in self.ids(P::Place) {
                if self.ts.u8(P::Place, town, 0x10) != 0 {
                    continue;
                }
                let id = i32::from(self.ts.u16(P::Place, town, 0));
                if chosen[..p].contains(&id) {
                    continue;
                }
                let w = self.town_value(town, company);
                let r = self.ts.rng.range(0x50, 0x78);
                let mut s = ftol(f64::from(r) * 0.01 * f64::from(w));
                let n = self.co_i32(CO_NET_STATIONS);
                if n <= 0 {
                    continue;
                }
                let tx = i32::from(self.ts.i16(P::Place, town, 4));
                let ty = i32::from(self.ts.i16(P::Place, town, 6));
                let mut d = 99_999;
                for k in 1..=n {
                    let st = self.network_station(k);
                    let sx = self.ts.i32(P::Station, st, 0xa);
                    let sy = self.ts.i32(P::Station, st, 0xe);
                    let dd = Ai::dist(tx, ty, sx, sy);
                    if d >= dd {
                        d = dd;
                    }
                }
                if d > 0x3c {
                    if d > 0x78 {
                        continue;
                    }
                    s >>= 2;
                    if d > 0x5a {
                        s >>= 2;
                    }
                }
                let d8 = d.max(8);
                let sc = ftol(30.0 / f64::from(d8 + 0xd) * f64::from(s));
                if sc > best {
                    best = sc;
                    s_best = s;
                    city = id;
                }
            }
            let x = ftol((cash_d + 2_000_000.0) * f64::from(best) * 4e-7);
            let mut budget = ftol(7.0 / f64::from(n_st + 7) * f64::from(x));
            if chair != 0 {
                let e = self.persona(chair, Trait::ExpandRail) + 900;
                budget = ftol(f64::from(e) * f64::from(budget) * 0.001);
            }
            match te {
                1 => budget = ftol(f64::from(budget) * 1.12),
                2 => budget = ftol(f64::from(budget) * 1.25),
                _ => {}
            }
            if city != 0 && budget > 300_000 {
                let mut c = 0i32;
                let r = self.connect_town(city, true, &mut c);
                cost[p] = c;
                result[p] = r;
                if f64::from(c) * 1.15 + 150_000.0 > cash_d || c > budget {
                    result[p] = false;
                }
                if result[p] {
                    viable += 1;
                }
                score[p] = s_best;
            }
            chosen[p] = city;
        }
        let mut k: Option<usize> = None;
        let mut best = 0f32;
        for p in 0..10 {
            if !result[p] {
                continue;
            }
            let q = f64::from(score[p]) / f64::from(cost[p]);
            if q > f64::from(best) {
                best = q as f32;
                k = Some(p);
            }
        }
        let Some(k) = k else {
            self.co_set_i32(CO_SATISFIED, 1);
            return false;
        };
        let mut dummy = 0;
        self.connect_town(chosen[k], false, &mut dummy);
        if cash > 1_000_000 {
            self.co_set_i32(CO_SATISFIED, 1);
        }
        true
    }

    /// `ai_plan_first_line(simulate)` (`0x435c20`, ai.md 5.3).
    pub fn plan_first_line(&mut self, simulate: bool) -> bool {
        self.ts.log.ran("ai_plan_first_line");
        if self.company != 0
            && self.cash() < 350_000.0
            && f64::from(self.ts.f32(P::Company, self.company, 0x4276)) < 0.7
        {
            return false;
        }
        let towns: Vec<i32> = self.ids(P::Place).into_iter().take(0x80).collect();
        let n = towns.len();
        let mut v = vec![0i32; n];
        let mut tx = vec![0i32; n];
        let mut ty = vec![0i32; n];
        let mut tid = vec![0i32; n];
        for (i, &t) in towns.iter().enumerate() {
            if self.ts.u8(P::Place, t, 0x10) == 0 && self.town_station_count(t) == 0 {
                let w = self.town_value(t, self.company) >> 10;
                let r = self.ts.rng.range(0x46, 0x82);
                v[i] = ftol(f64::from(r) * 0.01 * f64::from(w));
            }
            tx[i] = i32::from(self.ts.i16(P::Place, t, 4));
            ty[i] = i32::from(self.ts.i16(P::Place, t, 6));
            tid[i] = i32::from(self.ts.u16(P::Place, t, 0));
        }
        let mut m = vec![0i32; n * n];
        for i in 0..n {
            if v[i] <= 0 {
                continue;
            }
            for j in i + 1..n {
                if v[j] <= 0 || (tx[i] - tx[j]).abs() + (ty[i] - ty[j]).abs() >= 100 {
                    continue;
                }
                let mut d = Ai::dist(tx[i], ty[i], tx[j], ty[j]);
                if d > 0xc {
                    if d < 0x19 {
                        d = (d >> 2) + 0xf;
                    }
                    if d < 0x4b {
                        m[i * n + j] = v[i].wrapping_mul(v[j]) / d.wrapping_mul(d);
                    }
                }
            }
        }
        let mut used = vec![false; n];
        let mut active = [false; 20];
        let mut ok = [false; 20];
        let mut pa = [0usize; 20];
        let mut pb = [0usize; 20];
        let mut cost = [0i32; 20];
        let mut built = false;
        let mut last_k: Option<usize> = None;
        'outer: loop {
            let mut cont;
            loop {
                cont = 0;
                for slot in 0..20 {
                    if !active[slot] {
                        if cont < 3 {
                            active[slot] = true;
                            if n < 1 {
                                break 'outer;
                            }
                            let mut best = 0i32;
                            let (mut la, mut lb) = (0usize, 0usize);
                            for a in 0..n {
                                if used[a] {
                                    continue;
                                }
                                for b in a + 1..n {
                                    if !used[b] && best < m[a * n + b] {
                                        best = m[a * n + b];
                                        la = a;
                                        lb = b;
                                    }
                                }
                            }
                            if best < 1 {
                                break 'outer;
                            }
                            let mut c = 0;
                            let r = self.build_line(tid[la], tid[lb], true, Some(&mut c));
                            cost[slot] = c;
                            ok[slot] = r;
                            if r {
                                cont += 1;
                                pa[slot] = la;
                                pb[slot] = lb;
                            }
                            if self.ts.rng.range(1, 10) < 5 {
                                used[la] = true;
                            } else {
                                used[lb] = true;
                            }
                        } else {
                            active[slot] = false;
                        }
                    } else if ok[slot] {
                        cont += 1;
                    }
                }
                let mut k: Option<usize> = None;
                let mut best = 0f32;
                for slot in 0..20 {
                    if active[slot] && ok[slot] {
                        let q = f64::from(m[pa[slot] * n + pb[slot]]) / f64::from(cost[slot]);
                        if f64::from(best) < q {
                            best = q as f32;
                            k = Some(slot);
                        }
                    }
                }
                let Some(k) = k else {
                    break;
                };
                last_k = Some(k);
                built = self.build_line(tid[pa[k]], tid[pb[k]], simulate, None);
                if built {
                    break 'outer;
                }
                ok[k] = false;
            }
            if cont == 0 {
                break;
            }
        }
        if !simulate {
            let c = self.company;
            if self.ts.u8(P::Company, c, 0x4247) == 0 {
                self.ts.set8(P::Company, c, 0x4247, 1);
                if self.chairman() != 0 {
                    if built && let Some(k) = last_k {
                        self.company_name_from_towns(tid[pa[k]], tid[pb[k]]);
                    }
                    self.ts.log.ran("ai_news_founded");
                    self.ts
                        .g
                        .news
                        .push(crate::message::GameMessage::CompanyStarted {
                            company: self.company,
                        });
                }
            } else if built {
                self.ts.log.ran("ai_news_connected");
                let (from, to) = last_k.map_or((0, 0), |k| (tid[pa[k]] as i32, tid[pb[k]] as i32));
                self.ts
                    .g
                    .news
                    .push(crate::message::GameMessage::CompanyConnected {
                        company: self.company,
                        from,
                        to,
                    });
            }
        }
        built
    }

    /// `company_name_from_towns` (`0x43e040`): 1 or 2 draws; the name itself is not ported.
    fn company_name_from_towns(&mut self, _a: i32, _b: i32) {
        todo_hook!(
            self.ts.log,
            "company_name_from_towns",
            "company_name_from_towns 0x43e040 (RNG [0,100] once or twice, LNG 2281..2286)",
            "ai.md 5.3"
        );
    }
}
