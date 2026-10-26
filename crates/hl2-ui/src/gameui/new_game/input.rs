//! Shared toolkit focus, key activation, pointer capture and placement.
use super::{Dialog, Outcome, Target};
use glam::Vec2;
use source_ui::{
    input::{Input, Key},
    widgets::focus_step,
};

impl Dialog {
    pub fn take_sounds(&mut self) -> Vec<String> {
        std::mem::take(&mut self.sounds)
    }

    pub fn input(&mut self, input: &Input, canvas: Vec2) -> Outcome {
        let frame = self.frame(canvas);
        if self
            .placement
            .input(&frame, input.pointer, input.click, input.held, input.time)
        {
            self.tracker.occlude(input.click || input.held);
            self.keys.reset();
            return Outcome::None;
        }
        let origin = self.placement.place(&frame).origin;
        let pointer = input.pointer.map(|p| (p - origin) / frame.scale);
        self.hover =
            pointer.and_then(|p| self.targets().find(|(_, r)| r.contains(p)).map(|(t, _)| t));
        let clicked = self.tracker.update(
            self.hover,
            input.click || input.held,
            input.click,
            input.released,
        );
        if input.click {
            self.focus = self.hover;
            self.focus_visible = false;
        }
        if input.pressed(Key::Tab) {
            let mut order = [Target::Close; 8];
            let mut count = 0;
            for (target, _) in self.targets() {
                order[count] = target;
                count += 1;
            }
            self.focus = focus_step(self.focus, &order[..count], !input.shift);
            self.focus_visible = true;
        }
        if matches!(self.focus, Some(Target::Chapter(_))) {
            let delta = i32::from(input.pressed(Key::Right)) - i32::from(input.pressed(Key::Left));
            if delta != 0 && !self.catalog.entries.is_empty() {
                self.selected = (self.selected as i32 + delta)
                    .clamp(0, self.catalog.entries.len() as i32 - 1)
                    as usize;
                self.first = self.selected / 3 * 3;
                self.focus = Some(Target::Chapter(self.selected));
                self.focus_visible = true;
            }
        }
        let pressed = [Key::Enter, Key::Space]
            .into_iter()
            .find(|k| input.pressed(*k));
        let key = self
            .keys
            .update(self.focus, pressed, |k| input.held_keys.contains(&k));
        if let Some(target) = clicked.or(key) {
            self.sounds.push("ui/buttonclickrelease.wav".into());
            return self.activate(target, key.is_some());
        }
        Outcome::None
    }

    fn activate(&mut self, target: Target, keyboard: bool) -> Outcome {
        match target {
            Target::Close | Target::Cancel => return Outcome::Close,
            Target::Chapter(index) => {
                self.selected = index;
                if keyboard {
                    return self.start();
                }
            }
            Target::Next => {
                self.first += 3;
                self.selected = self.first;
                self.focus = Some(Target::Chapter(self.selected));
            }
            Target::Previous => {
                self.first = self.first.saturating_sub(3);
                self.selected = self.first;
                self.focus = Some(Target::Chapter(self.selected));
            }
            Target::Play => return self.start(),
        }
        Outcome::None
    }

    fn start(&self) -> Outcome {
        self.catalog
            .entries
            .get(self.selected)
            .map_or(Outcome::None, |entry| {
                Outcome::Start(crate::console::GameMap::new(&self.game, &entry.map))
            })
    }
}
