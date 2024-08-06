use core::str;
use std::collections::HashSet;

use crate::{Arg, Cmd, Pipeline};

#[derive(Default)]
pub(crate) struct SubscriptionTracker {
    subscriptions: HashSet<Vec<u8>>,
    s_subscriptions: HashSet<Vec<u8>>,
    p_subscriptions: HashSet<Vec<u8>>,
}

fn add_subscriptions<'a>(
    set: &mut HashSet<Vec<u8>>,
    iter: impl ExactSizeIterator<Item = Arg<&'a [u8]>>,
) {
    for arg in iter {
        if let Arg::Simple(channel) = arg {
            set.insert(channel.to_vec());
        }
    }
}

fn remove_subscriptions<'a>(
    set: &mut HashSet<Vec<u8>>,
    args_iter: impl ExactSizeIterator<Item = Arg<&'a [u8]>>,
) {
    for arg in args_iter {
        if let Arg::Simple(channel) = arg {
            set.remove(channel);
        }
    }
}

impl SubscriptionTracker {
    pub(crate) fn update_with_cmd<'a>(&'a mut self, cmd: &'a Cmd) {
        let mut args_iter = cmd.args_iter();
        let first_arg = args_iter.next();

        let Some(Arg::Simple(first_arg)) = first_arg else {
            return;
        };
        let Ok(first_arg) = str::from_utf8(first_arg) else {
            return;
        };

        if first_arg.eq_ignore_ascii_case("SUBSCRIBE") {
            add_subscriptions(&mut self.subscriptions, args_iter);
        } else if first_arg.eq_ignore_ascii_case("PSUBSCRIBE") {
            add_subscriptions(&mut self.p_subscriptions, args_iter);
        } else if first_arg.eq_ignore_ascii_case("SSUBSCRIBE") {
            add_subscriptions(&mut self.s_subscriptions, args_iter);
        } else if first_arg.eq_ignore_ascii_case("UNSUBSCRIBE") {
            remove_subscriptions(&mut self.subscriptions, args_iter);
        } else if first_arg.eq_ignore_ascii_case("PUNSUBSCRIBE") {
            remove_subscriptions(&mut self.p_subscriptions, args_iter);
        } else if first_arg.eq_ignore_ascii_case("SUNSUBSCRIBE") {
            remove_subscriptions(&mut self.s_subscriptions, args_iter);
        }
    }

    pub(crate) fn update_with_pipeline<'a>(&'a mut self, pipe: &'a Pipeline) {
        for cmd in pipe.cmd_iter() {
            self.update_with_cmd(cmd);
        }
    }

    pub(crate) fn get_subscription_pipeline(&self) -> Pipeline {
        let mut pipeline = crate::pipe();
        if !self.subscriptions.is_empty() {
            let cmd = pipeline.cmd("SUBSCRIBE");
            for channel in self.subscriptions.iter() {
                cmd.arg(channel);
            }
        }
        if !self.s_subscriptions.is_empty() {
            let cmd = pipeline.cmd("SSUBSCRIBE");
            for channel in self.s_subscriptions.iter() {
                cmd.arg(channel);
            }
        }
        if !self.p_subscriptions.is_empty() {
            let cmd = pipeline.cmd("PSUBSCRIBE");
            for channel in self.p_subscriptions.iter() {
                cmd.arg(channel);
            }
        }

        pipeline
    }
}

#[cfg(test)]
mod tests {
    use crate::{cmd, pipe};

    use super::*;

    #[test]
    fn test_add_and_remove_subscriptions() {
        let mut tracker = SubscriptionTracker::default();

        tracker.update_with_cmd(cmd("subscribe").arg("foo").arg("bar"));
        tracker.update_with_cmd(cmd("PSUBSCRIBE").arg("fo*o").arg("b*ar"));
        tracker.update_with_cmd(cmd("SSUBSCRIBE").arg("sfoo").arg("sbar"));
        tracker.update_with_cmd(cmd("unsubscribe").arg("foo"));
        tracker.update_with_cmd(cmd("Punsubscribe").arg("b*ar"));
        tracker.update_with_cmd(cmd("Sunsubscribe").arg("sfoo").arg("SBAR"));
        // ignore irrelevant commands
        tracker.update_with_cmd(cmd("GET").arg("sfoo"));

        let result = tracker.get_subscription_pipeline();
        let mut expected = pipe();
        expected
            .cmd("SUBSCRIBE")
            .arg("bar")
            .cmd("SSUBSCRIBE")
            .arg("sbar")
            .cmd("PSUBSCRIBE")
            .arg("fo*o");
        assert_eq!(
            result.get_packed_pipeline(),
            expected.get_packed_pipeline(),
            "{}",
            String::from_utf8(result.get_packed_pipeline()).unwrap()
        );
    }

    #[test]
    fn test_skip_empty_subscriptions() {
        let mut tracker = SubscriptionTracker::default();

        tracker.update_with_cmd(cmd("subscribe").arg("foo").arg("bar"));
        tracker.update_with_cmd(cmd("PSUBSCRIBE").arg("fo*o").arg("b*ar"));
        tracker.update_with_cmd(cmd("unsubscribe").arg("foo").arg("bar"));
        tracker.update_with_cmd(cmd("punsubscribe").arg("fo*o"));

        let result = tracker.get_subscription_pipeline();
        let mut expected = pipe();
        expected.cmd("PSUBSCRIBE").arg("b*ar");
        assert_eq!(
            result.get_packed_pipeline(),
            expected.get_packed_pipeline(),
            "{}",
            String::from_utf8(result.get_packed_pipeline()).unwrap()
        );
    }

    #[test]
    fn test_add_and_remove_subscriptions_with_pipeline() {
        let mut tracker = SubscriptionTracker::default();

        tracker.update_with_pipeline(
            pipe()
                .cmd("subscribe")
                .arg("foo")
                .arg("bar")
                .cmd("PSUBSCRIBE")
                .arg("fo*o")
                .arg("b*ar")
                .cmd("SSUBSCRIBE")
                .arg("sfoo")
                .arg("sbar")
                .cmd("unsubscribe")
                .arg("foo")
                .cmd("Punsubscribe")
                .arg("b*ar")
                .cmd("Sunsubscribe")
                .arg("sfoo")
                .arg("SBAR"),
        );

        let result = tracker.get_subscription_pipeline();
        let mut expected = pipe();
        expected
            .cmd("SUBSCRIBE")
            .arg("bar")
            .cmd("SSUBSCRIBE")
            .arg("sbar")
            .cmd("PSUBSCRIBE")
            .arg("fo*o");
        assert_eq!(
            result.get_packed_pipeline(),
            expected.get_packed_pipeline(),
            "{}",
            String::from_utf8(result.get_packed_pipeline()).unwrap()
        );
    }

    #[test]
    fn test_only_unsubscribe_from_existing_subscriptions() {
        let mut tracker = SubscriptionTracker::default();

        tracker.update_with_cmd(cmd("unsubscribe").arg("foo"));
        tracker.update_with_cmd(cmd("subscribe").arg("foo"));

        let result = tracker.get_subscription_pipeline();
        let mut expected = pipe();
        expected.cmd("SUBSCRIBE").arg("foo");
        assert_eq!(
            result.get_packed_pipeline(),
            expected.get_packed_pipeline(),
            "{}",
            String::from_utf8(result.get_packed_pipeline()).unwrap()
        );
    }
}
