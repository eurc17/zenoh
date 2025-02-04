//
// Copyright (c) 2022 ZettaScale Technology
//
// This program and the accompanying materials are made available under the
// terms of the Eclipse Public License 2.0 which is available at
// http://www.eclipse.org/legal/epl-2.0, or the Apache License, Version 2.0
// which is available at https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: EPL-2.0 OR Apache-2.0
//
// Contributors:
//   ZettaScale Zenoh Team, <zenoh@zettascale.tech>
//
use async_std::prelude::*;
use async_std::stream::StreamExt;
use zenoh::config::Config;
use zenoh::scouting::WhatAmI;

#[async_std::main]
async fn main() {
    // initiate logging
    env_logger::init();

    println!("Scouting...");
    let mut receiver = zenoh::scout(WhatAmI::Peer | WhatAmI::Router, Config::default())
        .await
        .unwrap();

    let scout = async {
        while let Some(hello) = receiver.next().await {
            println!("{}", hello);
        }
    };
    let timeout = async_std::task::sleep(std::time::Duration::from_secs(1));

    scout.race(timeout).await;

    // stop scouting
    drop(receiver);
}
