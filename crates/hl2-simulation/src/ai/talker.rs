//! The response system for expressive NPCs (SDK 2013 AI_ResponseSystem.cpp CResponseSystem,
//! scripts/talker/*.txt): enumerations, criteria, rules and weighted response groups. A
//! speaker builds a criteria set (concept, classname, health ...); the best-scoring rule
//! picks one of its response groups and the group picks a response.
use std::collections::{BTreeMap, BTreeSet};

/// CONCEPT_WEIGHT (baseentity.h): the concept and modifiers' criteria weight.
pub const CONCEPT_WEIGHT: f32 = 5.;

/// A criteria set: key -> (value, weight); keys compare case-insensitively.
#[derive(Clone, Debug, Default)]
pub struct CriteriaSet(BTreeMap<String, (String, f32)>);
impl CriteriaSet {
    /// AppendCriteria: a later value for the same key replaces the earlier one.
    pub fn append(&mut self, key: &str, value: impl Into<String>, weight: f32) {
        self.0
            .insert(key.to_ascii_lowercase(), (value.into(), weight));
    }
    pub fn get(&self, key: &str) -> Option<&(String, f32)> {
        self.0.get(&key.to_ascii_lowercase())
    }
}

/// ComputeMatcher's result for a criterion value (">5", "<=0.5", "!x", ">1,<3", "[enum::k]").
#[derive(Clone, Debug, Default)]
struct Matcher {
    usemin: bool,
    minequals: bool,
    minval: f32,
    usemax: bool,
    maxequals: bool,
    maxval: f32,
    notequal: bool,
    isnumeric: bool,
    token: String,
}

#[derive(Clone, Debug)]
struct Criterion {
    key: String,
    matcher: Matcher,
    weight: f32,
    required: bool,
    /// Subcriteria (a `{ a b c }` criterion): scored as their sum times the weight.
    sub: Vec<usize>,
}

/// Response types (ComputeResponseType).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResponseKind {
    Scene,
    Sentence,
    Speak,
    Response,
    Print,
}
#[derive(Clone, Debug)]
struct Response {
    kind: ResponseKind,
    value: String,
    weight: f32,
    depletion: u32,
    first: bool,
    last: bool,
}
/// AI_ResponseParams: the group's speech timing and options.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Params {
    /// RG_DELAYAFTERSPEAK: (start, range) seconds the speaker waits after speaking.
    pub delay: Option<(f32, f32)>,
    /// RG_DELAYBEFORESPEAK.
    pub predelay: Option<(f32, f32)>,
    pub speak_once: bool,
    /// RG_ODDS: percent chance to speak at all.
    pub odds: Option<i32>,
    /// RG_RESPEAKDELAY: the concept may not repeat before this.
    pub respeak_delay: Option<(f32, f32)>,
    pub soundlevel: Option<String>,
}
#[derive(Clone, Debug)]
struct Group {
    responses: Vec<Response>,
    params: Params,
    deplete_before_repeat: bool,
    sequential: bool,
    no_repeat: bool,
    enabled: bool,
    current: usize,
    depletion: u32,
}
impl Group {
    fn new() -> Self {
        Self {
            responses: Vec::new(),
            params: Params::default(),
            deplete_before_repeat: true,
            sequential: false,
            no_repeat: false,
            enabled: true,
            current: 0,
            depletion: 1,
        }
    }
    fn has_undepleted(&self) -> bool {
        !self.deplete_before_repeat || self.responses.iter().any(|r| r.depletion != self.depletion)
    }
    fn mark_used(&mut self, index: usize) {
        if self.deplete_before_repeat {
            self.responses[index].depletion = self.depletion;
        }
    }
    fn undepleted(&self, first: bool) -> Option<usize> {
        if !self.deplete_before_repeat {
            return None;
        }
        self.responses
            .iter()
            .position(|r| r.depletion != self.depletion && if first { r.first } else { r.last })
    }
}
#[derive(Clone, Debug)]
struct Rule {
    name: String,
    criteria: Vec<usize>,
    responses: Vec<usize>,
    match_once: bool,
    enabled: bool,
}
/// What FindBestResponse chose.
#[derive(Clone, Debug, PartialEq)]
pub struct Chosen {
    pub rule: String,
    pub kind: ResponseKind,
    pub value: String,
    pub params: Params,
}

/// The loaded rule database (CResponseSystem).
#[derive(Clone, Debug, Default)]
pub struct Talker {
    enumerations: BTreeMap<String, f32>,
    criteria: Vec<Criterion>,
    criteria_names: BTreeMap<String, usize>,
    groups: Vec<Group>,
    group_names: BTreeMap<String, usize>,
    rules: Vec<Rule>,
    pub warnings: Vec<String>,
}

/// The engine's ParseFile tokenizer: whitespace and // comments skipped, quoted strings,
/// single-character tokens for { } ( ) '.
struct Tokens<'a> {
    text: &'a [u8],
    at: usize,
    unget: Option<String>,
}
impl<'a> Tokens<'a> {
    fn next(&mut self) -> Option<String> {
        if let Some(t) = self.unget.take() {
            return Some(t);
        }
        let t = self.text;
        loop {
            while self.at < t.len() && t[self.at].is_ascii_whitespace() {
                self.at += 1;
            }
            if self.at + 1 < t.len() && t[self.at] == b'/' && t[self.at + 1] == b'/' {
                while self.at < t.len() && t[self.at] != b'\n' {
                    self.at += 1;
                }
                continue;
            }
            break;
        }
        if self.at >= t.len() {
            return None;
        }
        if t[self.at] == b'"' {
            self.at += 1;
            let start = self.at;
            while self.at < t.len() && t[self.at] != b'"' {
                self.at += 1;
            }
            let token = String::from_utf8_lossy(&t[start..self.at]).into_owned();
            self.at = (self.at + 1).min(t.len());
            return Some(token);
        }
        if b"{}()'".contains(&t[self.at]) {
            self.at += 1;
            return Some((t[self.at - 1] as char).to_string());
        }
        let start = self.at;
        while self.at < t.len()
            && !t[self.at].is_ascii_whitespace()
            && !b"{}()'\"".contains(&t[self.at])
        {
            self.at += 1;
        }
        Some(String::from_utf8_lossy(&t[start..self.at]).into_owned())
    }
    fn unget(&mut self, token: String) {
        self.unget = Some(token);
    }
    /// TokenWaiting: another token on the current line (a // comment ends the line).
    fn waiting(&self) -> bool {
        if self.unget.is_some() {
            return true;
        }
        let t = self.text;
        let mut p = self.at;
        while p < t.len() && t[p] != b'\n' {
            if t[p] == b'/' && t.get(p + 1) == Some(&b'/') {
                return false;
            }
            if !t[p].is_ascii_whitespace() {
                return true;
            }
            p += 1;
        }
        false
    }
}

fn root_command(token: &str) -> bool {
    [
        "#include",
        "response",
        "enumeration",
        "criteria",
        "criterion",
        "rule",
    ]
    .iter()
    .any(|c| token.eq_ignore_ascii_case(c))
}
/// ReadInterval: "a" or "a,b" -> (start, range).
fn interval(text: &str) -> (f32, f32) {
    let mut parts = text.split(',');
    let start = parts.next().map_or(0., atof);
    let range = parts.next().map_or(0., |b| atof(b) - start);
    (start, range)
}
/// C atof: the leading number, 0 when there is none.
fn atof(text: &str) -> f32 {
    let t = text.trim_start();
    let mut end = 0;
    let bytes = t.as_bytes();
    let mut seen_dot = false;
    let mut seen_exp = false;
    while end < bytes.len() {
        let c = bytes[end];
        let ok = c.is_ascii_digit()
            || ((c == b'-' || c == b'+') && (end == 0 || matches!(bytes[end - 1], b'e' | b'E')))
            || (c == b'.' && !seen_dot && !seen_exp)
            || ((c == b'e' || c == b'E') && !seen_exp && end > 0);
        if !ok {
            break;
        }
        seen_dot |= c == b'.';
        seen_exp |= c == b'e' || c == b'E';
        end += 1;
    }
    let mut slice = &t[..end];
    while !slice.is_empty() && slice.parse::<f32>().is_err() {
        slice = &slice[..slice.len() - 1];
    }
    slice.parse().unwrap_or(0.)
}
fn appears_numeric(token: &str) -> bool {
    atof(token) != 0. || token.chars().all(|c| c == '0')
}

impl Talker {
    /// LoadRuleSet: parse `path` and its #includes ("scripts/" + name) through `read`.
    pub fn load(path: &str, read: &dyn Fn(&str) -> Option<String>) -> Self {
        let mut talker = Self::default();
        let mut included = BTreeSet::new();
        talker.load_file(path, read, &mut included);
        talker
    }
    fn load_file(
        &mut self,
        path: &str,
        read: &dyn Fn(&str) -> Option<String>,
        included: &mut BTreeSet<String>,
    ) {
        let key = path.to_ascii_lowercase().replace('\\', "/");
        if !included.insert(key) {
            return;
        }
        let Some(text) = read(path) else {
            self.warnings.push(format!("unable to load {path}"));
            return;
        };
        let mut tokens = Tokens {
            text: text.as_bytes(),
            at: 0,
            unget: None,
        };
        while let Some(token) = tokens.next() {
            match token.to_ascii_lowercase().as_str() {
                "#include" => {
                    if let Some(file) = tokens.next() {
                        self.load_file(&format!("scripts/{file}"), read, included);
                    }
                }
                "response" => self.parse_response(&mut tokens),
                "criterion" | "criteria" => {
                    if let Some(name) = tokens.next() {
                        self.parse_criterion(&mut tokens, &name);
                    }
                }
                "rule" => self.parse_rule(&mut tokens),
                "enumeration" => self.parse_enumeration(&mut tokens),
                _ => {
                    self.warnings
                        .push(format!("{path}: unknown entry type '{token}'"));
                    break;
                }
            }
        }
    }
    fn resolve(&mut self, raw: &str) -> String {
        if !raw.starts_with('[') {
            return raw.to_owned();
        }
        match self.enumerations.get(&raw.to_ascii_lowercase()) {
            Some(v) => format!("{v:.6}"),
            None => {
                self.warnings.push(format!("no such enumeration '{raw}'"));
                raw.to_owned()
            }
        }
    }
    fn compute_matcher(&mut self, value: &str) -> Matcher {
        let mut m = Matcher::default();
        let (mut gt, mut lt, mut eq, mut nt) = (false, false, false, false);
        let mut raw = String::new();
        let mut token = String::new();
        let chars: Vec<char> = value.chars().chain(std::iter::once('\0')).collect();
        for c in chars {
            match c {
                '>' => gt = true,
                '<' => lt = true,
                '=' => eq = true,
                '!' => nt = true,
                ',' | '\0' => {
                    token = self.resolve(&raw);
                    raw.clear();
                    if gt {
                        m.usemin = true;
                        m.minequals = eq;
                        m.minval = atof(&token);
                        m.isnumeric = true;
                    } else if lt {
                        m.usemax = true;
                        m.maxequals = eq;
                        m.maxval = atof(&token);
                        m.isnumeric = true;
                    } else {
                        m.notequal = nt;
                        m.isnumeric = appears_numeric(&token);
                    }
                    gt = false;
                    lt = false;
                    eq = false;
                    nt = false;
                }
                c => raw.push(c),
            }
        }
        m.token = token;
        m
    }
    /// ParseOneCriterion: `key value [required] [weight n]` or `{ sub sub ... }`.
    fn parse_criterion(&mut self, tokens: &mut Tokens, name: &str) -> Option<usize> {
        let mut criterion = Criterion {
            key: String::new(),
            matcher: Matcher::default(),
            weight: 1.,
            required: false,
            sub: Vec::new(),
        };
        let mut value = String::new();
        let mut got_body = false;
        while tokens.waiting() || !got_body {
            let Some(token) = tokens.next() else { break };
            if root_command(&token) {
                tokens.unget(token);
                break;
            }
            if token == "{" {
                got_body = true;
                while let Some(sub) = tokens.next() {
                    if sub == "}" {
                        break;
                    }
                    match self.criteria_names.get(&sub.to_ascii_lowercase()) {
                        Some(&i) => criterion.sub.push(i),
                        None => self
                            .warnings
                            .push(format!("unknown subcriterion '{sub}' in '{name}'")),
                    }
                }
            } else if token.eq_ignore_ascii_case("required") {
                criterion.required = true;
            } else if token.eq_ignore_ascii_case("weight") {
                criterion.weight = tokens.next().map_or(1., |w| atof(&w));
            } else {
                criterion.key = token.to_ascii_lowercase();
                value = tokens.next().unwrap_or_default();
                got_body = true;
            }
        }
        if criterion.sub.is_empty() {
            criterion.matcher = self.compute_matcher(&value);
        }
        let key = name.to_ascii_lowercase();
        if self.criteria_names.contains_key(&key) {
            self.warnings
                .push(format!("multiple definitions for criteria '{name}'"));
            return None;
        }
        self.criteria.push(criterion);
        self.criteria_names.insert(key, self.criteria.len() - 1);
        Some(self.criteria.len() - 1)
    }
    fn parse_enumeration(&mut self, tokens: &mut Tokens) {
        let Some(name) = tokens.next() else { return };
        if tokens.next().as_deref() != Some("{") {
            self.warnings
                .push(format!("expecting '{{' in enumeration '{name}'"));
            return;
        }
        while let Some(key) = tokens.next() {
            if key == "}" || key.is_empty() {
                break;
            }
            let value = tokens.next().map_or(0., |v| atof(&v));
            self.enumerations
                .entry(format!("[{name}::{key}]").to_ascii_lowercase())
                .or_insert(value);
        }
    }
    /// The options shared by groups and single responses (ParseResponse/ParseOneResponse).
    fn parse_option(tokens: &mut Tokens, token: &str, params: &mut Params) -> bool {
        match token.to_ascii_lowercase().as_str() {
            "predelay" => params.predelay = tokens.next().map(|t| interval(&t)),
            "nodelay" => {
                // The SDK reads (and drops) the next token here.
                tokens.next();
                params.delay = Some((0., 0.));
            }
            // AIS_DEF_MIN_DELAY 2.8 .. AIS_DEF_MAX_DELAY 3.2.
            "defaultdelay" => params.delay = Some((2.8, 0.4)),
            "delay" => params.delay = tokens.next().map(|t| interval(&t)),
            "speakonce" => params.speak_once = true,
            "noscene" | "stop_on_nonidle" => {}
            "odds" => {
                params.odds = tokens.next().map(|t| (atof(&t) as i32).clamp(0, 100));
            }
            "respeakdelay" => params.respeak_delay = tokens.next().map(|t| interval(&t)),
            "weapondelay" => {
                tokens.next();
            }
            "soundlevel" => params.soundlevel = tokens.next(),
            _ => return false,
        }
        true
    }
    fn parse_one_response(&mut self, tokens: &mut Tokens, kind_token: &str, group: &mut Group) {
        let kind = match kind_token.to_ascii_lowercase().as_str() {
            "scene" => ResponseKind::Scene,
            "sentence" => ResponseKind::Sentence,
            "speak" => ResponseKind::Speak,
            "response" => ResponseKind::Response,
            "print" => ResponseKind::Print,
            _ => {
                self.warnings
                    .push(format!("unknown response type '{kind_token}'"));
                return;
            }
        };
        let mut response = Response {
            kind,
            value: tokens.next().unwrap_or_default(),
            weight: 1.,
            depletion: 0,
            first: false,
            last: false,
        };
        while tokens.waiting() {
            let Some(token) = tokens.next() else { break };
            if token.eq_ignore_ascii_case("weight") {
                response.weight = tokens.next().map_or(1., |w| atof(&w));
            } else if token.eq_ignore_ascii_case("displayfirst") {
                response.first = true;
            } else if token.eq_ignore_ascii_case("displaylast") {
                response.last = true;
            } else if !Self::parse_option(tokens, &token, &mut group.params) {
                self.warnings
                    .push(format!("unknown response command '{token}'"));
            }
        }
        group.responses.push(response);
    }
    fn parse_response(&mut self, tokens: &mut Tokens) {
        let Some(name) = tokens.next() else { return };
        let mut group = Group::new();
        while let Some(token) = tokens.next() {
            if root_command(&token) {
                tokens.unget(token);
                break;
            }
            if token == "{" {
                while let Some(token) = tokens.next() {
                    if token == "}" {
                        break;
                    }
                    match token.to_ascii_lowercase().as_str() {
                        "permitrepeats" => group.deplete_before_repeat = false,
                        "sequential" => group.sequential = true,
                        "norepeat" => group.no_repeat = true,
                        _ => self.parse_one_response(tokens, &token, &mut group),
                    }
                }
                break;
            }
            if Self::parse_option(tokens, &token, &mut group.params) {
                continue;
            }
            // A single-line response: `response name [options] type value`.
            self.parse_one_response(tokens, &token, &mut group);
            break;
        }
        self.groups.push(group);
        self.group_names
            .insert(name.to_ascii_lowercase(), self.groups.len() - 1);
    }
    fn parse_rule(&mut self, tokens: &mut Tokens) {
        let Some(name) = tokens.next() else { return };
        if tokens.next().as_deref() != Some("{") {
            self.warnings
                .push(format!("expecting '{{' in rule '{name}'"));
            return;
        }
        let mut rule = Rule {
            name: name.clone(),
            criteria: Vec::new(),
            responses: Vec::new(),
            match_once: false,
            enabled: true,
        };
        let mut valid = true;
        let mut inline = 0;
        while let Some(token) = tokens.next() {
            if token == "}" || token.is_empty() {
                break;
            }
            match token.to_ascii_lowercase().as_str() {
                "matchonce" => rule.match_once = true,
                "applycontexttoworld" => {}
                "applycontext" => {
                    tokens.next();
                }
                "response" => {
                    while tokens.waiting() {
                        let Some(r) = tokens.next() else { break };
                        match self.group_names.get(&r.to_ascii_lowercase()) {
                            Some(&i) => rule.responses.push(i),
                            None => {
                                valid = false;
                                self.warnings
                                    .push(format!("no such response '{r}' for rule '{name}'"));
                            }
                        }
                    }
                }
                "criteria" | "criterion" => {
                    while tokens.waiting() {
                        let Some(c) = tokens.next() else { break };
                        match self.criteria_names.get(&c.to_ascii_lowercase()) {
                            Some(&i) => rule.criteria.push(i),
                            None => {
                                valid = false;
                                self.warnings
                                    .push(format!("no such criterion '{c}' for rule '{name}'"));
                            }
                        }
                    }
                }
                _ => {
                    // An inline criterion.
                    inline += 1;
                    tokens.unget(token);
                    if let Some(i) = self.parse_criterion(tokens, &format!("[{name}{inline:03}]")) {
                        rule.criteria.push(i);
                    }
                }
            }
        }
        if valid {
            self.rules.push(rule);
        }
    }
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    fn compare(&self, value: &str, m: &Matcher) -> bool {
        let v = if value.starts_with('[') {
            self.enumerations
                .get(&value.to_ascii_lowercase())
                .copied()
                .unwrap_or(0.)
        } else {
            atof(value)
        };
        let mut minmax = 0;
        if m.usemin {
            if (m.minequals && v < m.minval) || (!m.minequals && v <= m.minval) {
                return false;
            }
            minmax += 1;
        }
        if m.usemax {
            if (m.maxequals && v > m.maxval) || (!m.maxequals && v >= m.maxval) {
                return false;
            }
            minmax += 1;
        }
        if minmax > 0 {
            return true;
        }
        if m.notequal {
            return if m.isnumeric {
                v != atof(&m.token)
            } else {
                !value.eq_ignore_ascii_case(&m.token)
            };
        }
        if m.isnumeric {
            // A missing key ("") must not match "0".
            return !value.is_empty() && v == atof(&m.token);
        }
        value.eq_ignore_ascii_case(&m.token)
    }
    /// ScoreCriteriaAgainstRuleCriteria: (score, exclude).
    fn score_criterion(&self, set: &CriteriaSet, index: usize) -> (f32, bool) {
        let c = &self.criteria[index];
        if !c.sub.is_empty() {
            let score: f32 = c.sub.iter().map(|&s| self.score_criterion(set, s).0).sum();
            return (score * c.weight, c.required && score == 0.);
        }
        let (value, weight) = set.get(&c.key).map_or(("", 1.), |(v, w)| (v.as_str(), *w));
        if self.compare(value, &c.matcher) {
            (weight * c.weight, false)
        } else {
            (0., c.required)
        }
    }
    fn score_rule(&self, set: &CriteriaSet, rule: &Rule) -> f32 {
        if !rule.enabled {
            return 0.;
        }
        let mut score = 0.;
        for &c in &rule.criteria {
            let (s, exclude) = self.score_criterion(set, c);
            score += s;
            if exclude {
                return 0.;
            }
        }
        score
    }
    /// SelectWeightedResponseFromResponseGroup (without a response filter).
    fn select_weighted(&mut self, group: usize, random: &mut dyn FnMut() -> f32) -> Option<usize> {
        let g = &mut self.groups[group];
        if g.responses.is_empty() {
            return None;
        }
        if !g.has_undepleted() {
            if g.deplete_before_repeat {
                g.depletion += 1;
            }
            if !g.has_undepleted() {
                return None;
            }
            if g.no_repeat {
                g.enabled = false;
                return None;
            }
        }
        let check = g.deplete_before_repeat;
        let mut slot = None;
        if check {
            slot = g.undepleted(true);
            if slot.is_none() {
                if let Some(last) = g.undepleted(false) {
                    let others = g
                        .responses
                        .iter()
                        .any(|r| r.depletion != g.depletion && !r.last);
                    if !others {
                        slot = Some(last);
                    }
                }
            }
        }
        if slot.is_none() {
            let mut total = 0.;
            for (i, r) in g.responses.iter().enumerate() {
                if check && (r.depletion == g.depletion || r.last) {
                    continue;
                }
                if total == 0. {
                    slot = Some(i);
                }
                total += r.weight;
                if total == 0. || random() * total < r.weight {
                    slot = Some(i);
                }
            }
        }
        if let Some(i) = slot {
            g.mark_used(i);
        }
        slot
    }
    /// GetBestResponse/ResolveResponse: a group's response, following `response` links.
    fn resolve_group(
        &mut self,
        group: usize,
        depth: usize,
        random: &mut dyn FnMut() -> f32,
    ) -> Option<(usize, usize)> {
        if depth > 8 || !self.groups[group].enabled || self.groups[group].responses.is_empty() {
            return None;
        }
        let index = if self.groups[group].sequential {
            let g = &mut self.groups[group];
            let mut i = g.current;
            g.current += 1;
            if i >= g.responses.len() {
                if g.no_repeat {
                    g.enabled = false;
                    return None;
                }
                i = 0;
                g.current = 1;
            }
            i
        } else {
            self.select_weighted(group, random)?
        };
        let r = &self.groups[group].responses[index];
        if r.kind == ResponseKind::Response {
            let target = *self.group_names.get(&r.value.to_ascii_lowercase())?;
            return self.resolve_group(target, depth + 1, random);
        }
        Some((group, index))
    }
    /// FindBestResponse: the best-scoring rule (ties at random), then one of its groups.
    pub fn find_best(
        &mut self,
        set: &CriteriaSet,
        random: &mut dyn FnMut() -> f32,
    ) -> Option<Chosen> {
        let mut best = Vec::new();
        let mut best_score = 0.001;
        for (i, rule) in self.rules.iter().enumerate() {
            let score = self.score_rule(set, rule);
            if score >= best_score {
                if score != best_score {
                    best_score = score;
                    best.clear();
                }
                best.push(i);
            }
        }
        if best.is_empty() {
            return None;
        }
        let pick =
            |n: usize, random: &mut dyn FnMut() -> f32| ((random() * n as f32) as usize).min(n - 1);
        let rule = best[pick(best.len(), random)];
        let groups = self.rules[rule].responses.clone();
        if self.rules[rule].match_once {
            self.rules[rule].enabled = false;
        }
        if groups.is_empty() {
            return None;
        }
        let group = groups[pick(groups.len(), random)];
        let (group, index) = self.resolve_group(group, 0, random)?;
        let r = &self.groups[group].responses[index];
        Some(Chosen {
            rule: self.rules[rule].name.clone(),
            kind: r.kind,
            value: r.value.clone(),
            params: self.groups[group].params.clone(),
        })
    }
    /// Every (kind, value) a rule matching `concept` can produce (precache).
    pub fn responses_for_concepts(&self, concepts: &[&str]) -> Vec<(ResponseKind, String)> {
        let mut out = Vec::new();
        for rule in &self.rules {
            let concept = rule.criteria.iter().any(|&c| {
                let c = &self.criteria[c];
                c.key == "concept"
                    && concepts
                        .iter()
                        .any(|k| k.eq_ignore_ascii_case(&c.matcher.token))
            });
            if !concept {
                continue;
            }
            let mut stack: Vec<(usize, usize)> = rule.responses.iter().map(|&g| (g, 0)).collect();
            while let Some((g, depth)) = stack.pop() {
                for r in &self.groups[g].responses {
                    if r.kind == ResponseKind::Response {
                        if let Some(&t) = self.group_names.get(&r.value.to_ascii_lowercase()) {
                            if depth < 8 {
                                stack.push((t, depth + 1));
                            }
                        }
                    } else {
                        out.push((r.kind, r.value.clone()));
                    }
                }
            }
        }
        out.sort_by(|a, b| a.1.cmp(&b.1));
        out.dedup_by(|a, b| a.1 == b.1);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const RULES: &str = r#"
enumeration "NPCState"
{
	"None"		"0"
	"Idle"		"1"
	"Combat"	"3"
}
criterion "IsCitizen" "classname" "npc_citizen" "required"
criterion "ConceptTalkWound" "Concept" "TLK_WOUND" required
criterion "NPCIdle" "npcstate" "[npcstate::idle]" required
criterion "HealthLow" "healthfrac" "<=0.25" required weight 2
criterion "IsBusy" { NPCIdle HealthLow } weight 3
response CitizenWound
{
	speak "npc_citizen.ow01"
	speak "npc_citizen.ow02" weight 3
}
response CitizenWoundLow respeakdelay 10,20 speak "npc_citizen.help01"
response CitizenAlias
{
	response CitizenWound
}
rule CitizenWound
{
	criteria		IsCitizen ConceptTalkWound
	response		CitizenWound
}
rule CitizenWoundLow
{
	criteria		IsCitizen ConceptTalkWound HealthLow
	response		CitizenWoundLow
}
rule CitizenInline
{
	criteria IsCitizen
	concept TLK_ALIAS required
	response CitizenAlias
}
"#;
    fn talker() -> Talker {
        Talker::load("scripts/talker/response_rules.txt", &|p| {
            (p == "scripts/talker/response_rules.txt").then(|| RULES.to_owned())
        })
    }
    fn set(concept: &str, health: &str) -> CriteriaSet {
        let mut s = CriteriaSet::default();
        s.append("concept", concept, CONCEPT_WEIGHT);
        s.append("classname", "npc_citizen", 1.);
        s.append("healthfrac", health, 1.);
        s.append("npcstate", "[NPCState::Idle]", 1.);
        s
    }
    #[test]
    fn rules_score_like_the_sdk_and_groups_deplete_before_repeating() {
        let mut t = talker();
        assert!(t.warnings.is_empty(), "{:?}", t.warnings);
        assert_eq!((t.rule_count(), t.group_count()), (3, 3));
        let mut rolls = [0.9f32, 0.1, 0.5, 0.5, 0.5, 0.5, 0.5].into_iter().cycle();
        let mut random = move || rolls.next().unwrap();
        // Full health: only CitizenWound matches (5 + 1); the 3x weighted entry first,
        // then the other one (depletion), then a new round.
        let a = t
            .find_best(&set("TLK_WOUND", "1.000"), &mut random)
            .unwrap();
        assert_eq!(a.rule, "CitizenWound");
        let b = t
            .find_best(&set("TLK_WOUND", "1.000"), &mut random)
            .unwrap();
        assert_ne!(a.value, b.value);
        // Low health matches the required HealthLow (weight 2): that rule scores 8.
        let low = t
            .find_best(&set("TLK_WOUND", "0.200"), &mut random)
            .unwrap();
        assert_eq!(
            (
                low.rule.as_str(),
                low.value.as_str(),
                low.params.respeak_delay
            ),
            ("CitizenWoundLow", "npc_citizen.help01", Some((10., 10.)))
        );
        // Wrong class or concept: nothing (required criteria exclude the rule).
        let mut other = set("TLK_WOUND", "1.000");
        other.append("classname", "npc_barney", 1.);
        assert_eq!(t.find_best(&other, &mut random), None);
        // Inline criteria and response links resolve to the linked group.
        let alias = t
            .find_best(&set("TLK_ALIAS", "1.000"), &mut random)
            .unwrap();
        assert_eq!(alias.rule, "CitizenInline");
        assert!(alias.value.starts_with("npc_citizen.ow"));
        // Subcriteria score their sum times the weight; enumerations resolve.
        let busy = *t.criteria_names.get("isbusy").unwrap();
        assert_eq!(t.score_criterion(&set("X", "0.100"), busy), (9., false));
        assert_eq!(
            t.responses_for_concepts(&["TLK_WOUND"])
                .into_iter()
                .map(|r| r.1)
                .collect::<Vec<_>>(),
            ["npc_citizen.help01", "npc_citizen.ow01", "npc_citizen.ow02"]
        );
    }
    #[test]
    fn matchers_follow_compare_using_matcher() {
        let mut t = Talker::default();
        let m = t.compute_matcher(">0.5,<=2");
        assert!(t.compare("1", &m) && t.compare("2", &m) && !t.compare("0.5", &m));
        let m = t.compute_matcher("!npc_combine_s");
        assert!(t.compare("npc_citizen", &m) && !t.compare("NPC_Combine_S", &m));
        let m = t.compute_matcher("0");
        assert!(t.compare("0", &m) && !t.compare("", &m));
        assert_eq!(atof("3.5abc"), 3.5);
        assert_eq!(interval("10,20"), (10., 10.));
    }
}
