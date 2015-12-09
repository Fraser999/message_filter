// Copyright 2015 MaidSafe.net limited.
//
// This SAFE Network Software is licensed to you under (1) the MaidSafe.net Commercial License,
// version 1.0 or later, or (2) The General Public License (GPL), version 3, depending on which
// licence you accepted on initial access to the Software (the "Licences").
//
// By contributing code to the SAFE Network Software, or to this project generally, you agree to be
// bound by the terms of the MaidSafe Contributor Agreement, version 1.0.  This, along with the
// Licenses can be found in the root directory of this project at LICENSE, COPYING and CONTRIBUTOR.
//
// Unless required by applicable law or agreed to in writing, the SAFE Network Software distributed
// under the GPL Licence is distributed on an "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.
//
// Please review the Licences for the specific language governing permissions and limitations
// relating to use of the SAFE Network Software.

//! # Message Filter
//!
//! A size or time based message filter that takes any generic type as a key and will drop keys
//! after a time period, or once a maximum number of messages is reached (LRU Cache pattern).  The
//! filter currently only allows adding messages; a delete function will be provided at a later
//! stage.
//!
//! This library can be used by network based systems to filter previously seen messages.
//!
//! # Examples
//!
//! ```
//! # #![allow(unused_variables)]
//! # extern crate message_filter;
//! # extern crate time;
//! # fn main() {
//! use ::message_filter::MessageFilter;
//!
//! // Construct a `MessageFilter` of `u8`s, limited by message count
//! let max_count = 10;
//! let message_filter = MessageFilter::<u8>::with_capacity(max_count);
//!
//! // Construct a `MessageFilter` of `String`s, limited by expiry time
//! let time_to_live = ::time::Duration::milliseconds(100);
//! let message_filter = MessageFilter::<String>::with_expiry_duration(time_to_live);
//!
//! // Construct a `MessageFilter` of `Vec<u8>`s, limited by message count and expiry time
//! let message_filter = MessageFilter::<Vec<u8>>::with_expiry_duration_and_capacity(time_to_live,
//!                                                                                  max_count);
//! # }
//! ```

#![doc(html_logo_url =
           "https://raw.githubusercontent.com/maidsafe/QA/master/Images/maidsafe_logo.png",
       html_favicon_url = "http://maidsafe.net/img/favicon.ico",
       html_root_url = "http://maidsafe.github.io/message_filter")]

// For explanation of lint checks, run `rustc -W help` or see
// https://github.com/maidsafe/QA/blob/master/Documentation/Rust%20Lint%20Checks.md
#![forbid(bad_style, exceeding_bitshifts, mutable_transmutes, no_mangle_const_items,
          unknown_crate_types, warnings)]
#![deny(deprecated, drop_with_repr_extern, improper_ctypes, missing_docs,
        non_shorthand_field_patterns, overflowing_literals, plugin_as_library,
        private_no_mangle_fns, private_no_mangle_statics, stable_features, unconditional_recursion,
        unknown_lints, unsafe_code, unused, unused_allocation, unused_attributes,
        unused_comparisons, unused_features, unused_parens, while_true)]
#![warn(trivial_casts, trivial_numeric_casts, unused_extern_crates, unused_import_braces,
        unused_qualifications, unused_results, variant_size_differences)]
#![allow(box_pointers, fat_ptr_transmutes, missing_copy_implementations,
         missing_debug_implementations)]

#[cfg(test)]
extern crate rand;
extern crate time;

/// Implementation of [message filter](index.html#message-filter).
pub struct MessageFilter<Message> where Message: Ord + Clone {
    entries: Vec<TimestampedMessage<Message>>,
    capacity: Option<usize>,
    time_to_live: Option<::time::Duration>,
}

impl<Message> MessageFilter<Message> where Message: Ord + Clone {
    /// Constructor for capacity based `MessageFilter`.
    pub fn with_capacity(capacity: usize) -> MessageFilter<Message> {
        MessageFilter {
            entries: vec![],
            capacity: Some(capacity),
            time_to_live: None,
        }
    }

    /// Constructor for time based `MessageFilter`.
    pub fn with_expiry_duration(time_to_live: ::time::Duration) -> MessageFilter<Message> {
        MessageFilter {
            entries: vec![],
            capacity: None,
            time_to_live: Some(time_to_live),
        }
    }

    /// Constructor for dual-feature capacity and time based `MessageFilter`.
    pub fn with_expiry_duration_and_capacity(time_to_live: ::time::Duration,
                                             capacity: usize)
                                             -> MessageFilter<Message> {
        MessageFilter {
            entries: vec![],
            capacity: Some(capacity),
            time_to_live: Some(time_to_live),
        }
    }

    /// Removes any expired messages, then adds `message`, then removes enough older messages until
    /// the message count is at or below `capacity`.  If `message` already exists in the filter and
    /// is not already expired, its original expiry time and place in the FIFO queue remains
    /// unchanged by this call.
    pub fn insert(&mut self, message: Message) -> Option<Message> {
        if !self.contains(&message) {  // This call prunes expired entries
            self.entries.push(TimestampedMessage::new(message, self.time_to_live));
            self.remove_excess();
            None
        } else {
            if self.time_to_live.is_none() {
                return Some(message.clone())
            }
            if let Some(index) = self.entries.iter().position(|ref t| t.message == message) {
                let timestamped_message = self.entries.remove(index);
                self.entries.push(TimestampedMessage::new(message, self.time_to_live));
                return Some(timestamped_message.message)
            }
            None
        }
    }

    /// Removes any expired messages, then returns whether `message` exists in the filter or not.
    pub fn contains(&mut self, message: &Message) -> bool {
        self.remove_expired();
        self.entries.iter().any(|ref entry| entry.message == *message)
    }

    /// Returns the size of the filter, i.e. the number of added messages.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    fn remove_excess(&mut self) {
        // If capacity is Some, remove the first entry if we're above the limit (should only ever be
        // at most one entry above capacity).
        if let Some(capacity) = self.capacity {
            if self.entries.len() > capacity {
                let _ = self.entries.remove(0);
                debug_assert!(self.entries.len() == capacity);
            }
        }
    }

    fn remove_expired(&mut self) {
        if self.time_to_live.is_some() {
            let now = ::time::SteadyTime::now();
            // The entries are sorted from oldest to newest, so just split off the vector at the
            // first unexpired entry and the returned vector is the remaining unexpired values.  If
            // we don't find any unexpired value, just clear the vector.
            if let Some(at) = self.entries.iter().position(|ref entry| entry.expiry_point > now) {
                self.entries = self.entries.split_off(at)
            } else {
                self.entries.clear();
            }
        }
    }
}

struct TimestampedMessage<Message> {
    pub message: Message,
    pub expiry_point: ::time::SteadyTime,
}

impl<Message> TimestampedMessage<Message> {
    pub fn new(message: Message,
               time_to_live: Option<::time::Duration>)
               -> TimestampedMessage<Message>{
        TimestampedMessage {
            message: message,
            expiry_point: match time_to_live {
                              Some(time_to_live) => ::time::SteadyTime::now() + time_to_live,
                              None => ::time::SteadyTime::now(),
                          },
        }
    }
}



#[cfg(test)]
mod test {
    #[test]
    fn size_only() {
        let size = ::rand::random::<u8>() as usize + 1;
        let mut msg_filter = super::MessageFilter::<usize>::with_capacity(size);
        assert!(msg_filter.time_to_live.is_none());
        assert_eq!(Some(size), msg_filter.capacity);

        // Add `size` messages - all should be added.
        for i in 0..size {
            assert_eq!(msg_filter.len(), i);
            let element = msg_filter.insert(i);
            assert_eq!(element, None);
            assert_eq!(msg_filter.len(), i + 1);
        }

        // Check all added messages remain.
        assert!((0..size).all(|index| msg_filter.contains(&index)));

        // Add further messages - all should be added, each time pushing out the oldest message.
        for i in size..1000 {
            let _ = msg_filter.insert(i);
            assert_eq!(msg_filter.len(), size);
            assert!(msg_filter.contains(&i));
            if size > 1 {
                assert!(msg_filter.contains(&(i - 1)));
                assert!(msg_filter.contains(&(i - size + 1)));
            }
            assert!(!msg_filter.contains(&(i - size)));
        }
    }

    #[test]
    fn time_only() {
        use ::rand::Rng;
        let time_to_live = ::time::Duration::milliseconds(::rand::thread_rng().gen_range(50, 150));
        let mut msg_filter = super::MessageFilter::<usize>::with_expiry_duration(time_to_live);
        assert_eq!(Some(time_to_live), msg_filter.time_to_live);
        assert_eq!(None, msg_filter.capacity);

        // Add 10 messages - all should be added.
        for i in 0..10 {
            let element = msg_filter.insert(i);
            assert_eq!(element, None);
            assert!(msg_filter.contains(&i));
        }
        assert_eq!(msg_filter.len(), 10);

        // Allow the added messages time to expire.
        let sleep_duration =
            ::std::time::Duration::from_millis(time_to_live.num_milliseconds() as u64 + 10);
        ::std::thread::sleep(sleep_duration);

        // Add a new message which should cause the expired values to be removed.
        let _ = msg_filter.insert(11);
        assert!(msg_filter.contains(&11));
        assert_eq!(msg_filter.len(), 1);

        // Check we can add the initial messages again.
        for i in 0..10 {
            assert_eq!(msg_filter.len(), i + 1);
            let _ = msg_filter.insert(i);
            assert!(msg_filter.contains(&i));
            assert_eq!(msg_filter.len(), i + 2);
        }
    }

    #[test]
    fn time_and_size() {
        use ::rand::Rng;
        let size = ::rand::random::<u8>() as usize + 1;
        let time_to_live = ::time::Duration::milliseconds(::rand::thread_rng().gen_range(50, 150));
        let mut msg_filter =
            super::MessageFilter::<usize>::with_expiry_duration_and_capacity(time_to_live, size);
        assert_eq!(Some(time_to_live), msg_filter.time_to_live);
        assert_eq!(Some(size), msg_filter.capacity);

        for i in 0..1000 {
            // Check `size` has not been exceeded.
            if i < size {
                assert_eq!(msg_filter.len(), i);
            } else {
                assert_eq!(msg_filter.len(), size);
            }

            // Add a new message and check that it has been added successfully.
            let element = msg_filter.insert(i);
            assert_eq!(element, None);
            assert!(msg_filter.contains(&i));

            // Check `size` has not been exceeded.
            if i < size {
                assert_eq!(msg_filter.len(), i + 1);
            } else {
                assert_eq!(msg_filter.len(), size);
            }
        }

        // Allow the added messages time to expire.
        let sleep_duration = ::std::time::Duration::from_millis(
                                 time_to_live.num_milliseconds() as u64 + 10);
        ::std::thread::sleep(sleep_duration);

        // Check for the last message, which should cause all the values to be removed.
        assert!(!msg_filter.contains(&1000));
        assert_eq!(msg_filter.len(), 0);
    }

    #[test]
    fn time_size_struct_value() {
        use ::rand::Rng;

        #[derive(PartialEq, PartialOrd, Ord, Clone, Eq, Hash)]
        struct Temp {
            id: Vec<u8>,
        }

        impl Temp {
            fn new() -> Temp {
                let mut rng = ::rand::thread_rng();
                Temp { id: ::rand::sample(&mut rng, 0u8..255, 64) }
            }
        }

        let size = ::rand::random::<u8>() as usize + 1;
        let time_to_live = ::time::Duration::milliseconds(::rand::thread_rng().gen_range(50, 150));
        let mut msg_filter =
            super::MessageFilter::<Temp>::with_expiry_duration_and_capacity(time_to_live, size);
        assert_eq!(Some(time_to_live), msg_filter.time_to_live);
        assert_eq!(Some(size), msg_filter.capacity);

        for i in 0..1000 {
            // Check `size` has not been exceeded.
            if i < size {
                assert_eq!(msg_filter.len(), i);
            } else {
                assert_eq!(msg_filter.len(), size);
            }

            // Add a new message and check that it has been added successfully.
            let temp = Temp::new();
            let _ = msg_filter.insert(temp.clone());
            assert!(msg_filter.contains(&temp));

            // Check `size` has not been exceeded.
            if i < size {
                assert_eq!(msg_filter.len(), i + 1);
            } else {
                assert_eq!(msg_filter.len(), size);
            }
        }

        // Allow the added messages time to expire.
        let sleep_duration =
            ::std::time::Duration::from_millis(time_to_live.num_milliseconds() as u64 + 10);
        ::std::thread::sleep(sleep_duration);

        // Add a new message which should cause the expired values to be removed.
        let temp = Temp::new();
        let _ = msg_filter.insert(temp.clone());
        assert_eq!(msg_filter.len(), 1);
        assert!(msg_filter.contains(&temp));
    }

    #[test]
    fn add_duplicate() {
        // Check re-adding a message to a capacity-based filter doesn't alter its position in the
        // FIFO queue.
        let size = 3;
        let mut capacity_filter = super::MessageFilter::<usize>::with_capacity(size);

        // Add `size` messages - all should be added.
        for i in 0..size {
            let element = capacity_filter.insert(i);
            assert_eq!(element, None);
        }

        // Check all added messages remain.
        assert!((0..size).all(|index| capacity_filter.contains(&index)));

        // Add "0" again.
        let element = capacity_filter.insert(0);
        assert!(element.is_some());
        assert_eq!(element.unwrap(), 0);

        // Add "3" and check it's pushed out "0".
        let element = capacity_filter.insert(3);
        assert_eq!(element, None);
        assert!(!capacity_filter.contains(&0));
        assert!(capacity_filter.contains(&1));
        assert!(capacity_filter.contains(&2));
        assert!(capacity_filter.contains(&3));

        // Check re-adding a message to a time-based filter alter's its expiry time.
        let time_to_live = ::time::Duration::milliseconds(200);
        let mut time_filter = super::MessageFilter::<usize>::with_expiry_duration(time_to_live);

        // Add "0".
        let element = time_filter.insert(0);
        assert_eq!(element, None);

        // Wait for half the expiry time and re-add "0".
        let sleep_duration =
            ::std::time::Duration::from_millis((time_to_live.num_milliseconds() as u64 / 2) + 10);
        ::std::thread::sleep(sleep_duration);
        let element = time_filter.insert(0);
        assert!(element.is_some());
        assert_eq!(element.unwrap(), 0);

        // Wait for another half of the expiry time and check it's not been removed.
        ::std::thread::sleep(sleep_duration);
        assert!(time_filter.contains(&0));

        // Wait for another half of the expiry time and check it's been removed.
        ::std::thread::sleep(sleep_duration);
        assert!(!time_filter.contains(&0));
    }
}
