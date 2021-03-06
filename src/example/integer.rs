use justification::LatestMsgsHonest;
use message::{CasperMsg, Message};
use senders_weight::SendersWeight;
use std::collections::HashSet;
use std::iter::FromIterator;
use traits::{Estimate, Zero};
use weight_unit::WeightUnit;
type Validator = u32;

#[derive(Clone, Eq, Debug, Ord, PartialOrd, PartialEq, Hash, serde_derive::Serialize)]
pub struct IntegerWrapper(u32);

impl IntegerWrapper {
    pub fn new(estimate: u32) -> Self {
        IntegerWrapper(estimate)
    }
}

#[cfg(feature = "integration_test")]
impl<S: ::traits::Sender> From<S> for IntegerWrapper {
    fn from(_sender: S) -> Self {
        IntegerWrapper::new(u32::default())
    }
}

pub type IntegerMsg = Message<IntegerWrapper /*Estimate*/, Validator /*Sender*/>;

#[derive(Clone, Eq, Debug, Ord, PartialOrd, PartialEq, Hash)]
pub struct Tx;

/// the goal here is to find the weighted median of all the values
impl Estimate for IntegerWrapper {
    type M = IntegerMsg;

    fn mk_estimate(
        latest_msgs: &LatestMsgsHonest<Self::M>,
        senders_weights: &SendersWeight<<<Self as Estimate>::M as CasperMsg>::Sender>,
    ) -> Result<Self, &'static str> {
        let mut msgs_sorted_by_estimate = Vec::from_iter(latest_msgs.iter().fold(
            HashSet::new(),
            |mut latest, latest_from_validator| {
                latest.insert(latest_from_validator);
                latest
            },
        ));
        msgs_sorted_by_estimate.sort_unstable_by(|a, b| a.estimate().cmp(&b.estimate()));

        // get the total weight of the senders of the messages
        // in the set
        let total_weight = msgs_sorted_by_estimate
            .iter()
            .fold(WeightUnit::ZERO, |acc, x| {
                acc + senders_weights
                    .weight(x.sender())
                    .unwrap_or(WeightUnit::ZERO)
            });

        let mut running_weight = 0.0;
        let mut msg_iter = msgs_sorted_by_estimate.iter();
        let mut current_msg: Result<&&IntegerMsg, &str> = Err("no msg");

        // since the messages are ordered according to their estimates,
        // whichever estimate is found after iterating over half of the total weight
        // is the consensus
        while running_weight / total_weight < 0.5 {
            current_msg = msg_iter.next().ok_or("no next msg");
            running_weight += current_msg
                .and_then(|m| senders_weights.weight(m.sender()))
                .unwrap_or(WeightUnit::ZERO)
        }

        // return said estimate
        current_msg.map(|m| m.estimate().clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use justification::{Justification, LatestMsgs, SenderState};
    use senders_weight::SendersWeight;

    #[test]
    fn equal_weight() {
        let senders: Vec<u32> = (0..4).collect();
        let weights = [1.0, 1.0, 1.0, 1.0];

        let senders_weights = SendersWeight::new(
            senders
                .iter()
                .cloned()
                .zip(weights.iter().cloned())
                .collect(),
        );

        let sender_state = SenderState::new(
            senders_weights.clone(),
            0.0, // state fault weight
            None,
            LatestMsgs::new(),
            1.0,            // subjective fault weight threshold
            HashSet::new(), // equivocators
        );

        assert_eq!(
            IntegerWrapper::mk_estimate(
                &LatestMsgsHonest::from_latest_msgs(
                    &LatestMsgs::from(&Justification::new()),
                    sender_state.equivocators()
                ),
                &senders_weights,
            ),
            Err("no msg")
        );

        let m0 = IntegerMsg::new(senders[0], Justification::new(), IntegerWrapper(1), None);
        let m1 = IntegerMsg::new(senders[1], Justification::new(), IntegerWrapper(2), None);
        let m2 = IntegerMsg::new(senders[2], Justification::new(), IntegerWrapper(3), None);
        let (m3, _) = IntegerMsg::from_msgs(senders[0], vec![&m0, &m1], &sender_state).unwrap();

        let (mut j0, _) = Justification::from_msgs(vec![m0.clone(), m1.clone()], &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(1)
        );

        j0.faulty_insert(&m2, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(2)
        );

        j0.faulty_insert(&m3, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(2)
        );
    }

    /// the 1st validator has most of the weight
    #[test]
    fn uneven_weights_1() {
        let senders: Vec<u32> = (0..4).collect();
        let weights = [4.0, 1.0, 1.0, 1.0];

        let senders_weights = SendersWeight::new(
            senders
                .iter()
                .cloned()
                .zip(weights.iter().cloned())
                .collect(),
        );

        let sender_state = SenderState::new(
            senders_weights.clone(),
            0.0, // state fault weight
            None,
            LatestMsgs::new(),
            1.0,            // subjective fault weight threshold
            HashSet::new(), // equivocators
        );

        assert_eq!(
            IntegerWrapper::mk_estimate(
                &LatestMsgsHonest::from_latest_msgs(
                    &LatestMsgs::from(&Justification::new()),
                    sender_state.equivocators()
                ),
                &senders_weights,
            ),
            Err("no msg")
        );

        let m0 = IntegerMsg::new(senders[0], Justification::new(), IntegerWrapper(1), None);
        let m1 = IntegerMsg::new(senders[1], Justification::new(), IntegerWrapper(2), None);
        let m2 = IntegerMsg::new(senders[2], Justification::new(), IntegerWrapper(3), None);
        let (m3, _) = IntegerMsg::from_msgs(senders[0], vec![&m0, &m1], &sender_state).unwrap();

        let (mut j0, _) = Justification::from_msgs(vec![m0.clone(), m1.clone()], &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(1)
        );

        j0.faulty_insert(&m2, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(1)
        );

        j0.faulty_insert(&m3, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(1)
        );
    }

    /// the 4th validator has most of the weight
    #[test]
    fn uneven_weights_4() {
        let senders: Vec<u32> = (0..4).collect();
        let weights = [1.0, 1.0, 1.0, 4.0];

        let senders_weights = SendersWeight::new(
            senders
                .iter()
                .cloned()
                .zip(weights.iter().cloned())
                .collect(),
        );

        let sender_state = SenderState::new(
            senders_weights.clone(),
            0.0, // state fault weight
            None,
            LatestMsgs::new(),
            1.0,            // subjective fault weight threshold
            HashSet::new(), // equivocators
        );

        assert_eq!(
            IntegerWrapper::mk_estimate(
                &LatestMsgsHonest::from_latest_msgs(
                    &LatestMsgs::from(&Justification::new()),
                    sender_state.equivocators()
                ),
                &senders_weights,
            ),
            Err("no msg")
        );

        let m0 = IntegerMsg::new(senders[0], Justification::new(), IntegerWrapper(1), None);
        let m1 = IntegerMsg::new(senders[1], Justification::new(), IntegerWrapper(2), None);
        let m2 = IntegerMsg::new(senders[2], Justification::new(), IntegerWrapper(3), None);
        let m3 = IntegerMsg::new(senders[3], Justification::new(), IntegerWrapper(4), None);

        let (m4, _) =
            IntegerMsg::from_msgs(senders[3], vec![&m0, &m1, &m2, &m3], &sender_state).unwrap();

        let (mut j0, _) = Justification::from_msgs(vec![m0.clone(), m1.clone()], &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(1)
        );

        j0.faulty_insert(&m2, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(2)
        );

        j0.faulty_insert(&m3, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(4)
        );

        j0.faulty_insert(&m4, &sender_state);
        assert_eq!(
            j0.mk_estimate(sender_state.equivocators(), &senders_weights)
                .unwrap(),
            IntegerWrapper(4)
        );
    }
}
