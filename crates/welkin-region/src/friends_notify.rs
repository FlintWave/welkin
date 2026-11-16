// SPDX-License-Identifier: LGPL-3.0-or-later

//! Friend online/offline notifications, delivered through the region egress.

use uuid::Uuid;
use welkin_modules::egress::{ClientEgress, EgressEvent};
use welkin_services::remote::friends::RemoteFriendsConnector;

/// Right granted to a friend: "can see my online status" (OpenSim `FriendRights.CanSeeOnline`).
const CAN_SEE_ONLINE: i32 = 1;

/// Tell `agent`'s friends that they went online or offline (only those they let see their
/// status). When coming online, also tell `agent` which of their friends are already here.
pub async fn notify_status(
    egress: &dyn ClientEgress,
    friends: &RemoteFriendsConnector,
    agent: Uuid,
    online: bool,
) {
    let records = match friends.get_friends(agent).await {
        Ok(records) => records,
        Err(err) => {
            tracing::warn!(%agent, error = %err, "friend notifications: could not load the friends list");
            return;
        }
    };
    for record in records {
        if record.my_flags & CAN_SEE_ONLINE != 0 {
            egress.send(EgressEvent::FriendOnline {
                agent: record.friend_id,
                friend: agent,
                online,
            });
        }
        if online && record.their_flags & CAN_SEE_ONLINE != 0 && egress.is_connected(record.friend_id) {
            egress.send(EgressEvent::FriendOnline {
                agent,
                friend: record.friend_id,
                online: true,
            });
        }
    }
}
