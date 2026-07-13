// Task Tracker is a self-hosted task and kanban board application.
// Copyright 2026-present Task Tracker and contributors. All rights reserved.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

package models

import (
	"time"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/cron"
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/notifications"
	"github.com/FerrPOINT/task-tracker/pkg/user"

	"xorm.io/builder"
	"xorm.io/xorm"
)

// RegisterAPITokenExpiryCheckCron registers the cron job that checks for
// expiring API tokens and notifies their owners.
func RegisterAPITokenExpiryCheckCron() {
	if !config.MailerEnabled.GetBool() {
		return
	}

	err := cron.Schedule("0 * * * *", func() { checkForExpiringAPITokensAt(time.Now()) })
	if err != nil {
		log.Fatalf("Could not register API token expiry check cron: %s", err)
	}
}

func checkForExpiringAPITokensAt(now time.Time) {
	const logPrefix = "[API Token Expiry Check] "

	oneDay := now.Add(24 * time.Hour)
	sevenDays := now.Add(7 * 24 * time.Hour)

	s := db.NewSession()
	defer s.Close()

	// Find all tokens expiring within the next 7 days that haven't expired yet
	var tokens []*APIToken
	err := s.Where(
		builder.Gt{"expires_at": now},
	).And(
		builder.Lte{"expires_at": sevenDays},
	).Find(&tokens)
	if err != nil {
		log.Errorf(logPrefix+"Error getting expiring tokens: %s", err)
		return
	}

	if len(tokens) == 0 {
		return
	}

	log.Debugf(logPrefix+"Found %d tokens expiring within 7 days", len(tokens))

	// Collect unique owner IDs and fetch users
	ownerIDs := make([]int64, 0, len(tokens))
	for _, token := range tokens {
		ownerIDs = append(ownerIDs, token.OwnerID)
	}

	users, err := user.GetUsersByIDs(s, ownerIDs)
	if err != nil {
		log.Errorf(logPrefix+"Error getting token owners: %s", err)
		return
	}

	for _, token := range tokens {
		u, exists := users[token.OwnerID]
		if !exists {
			continue
		}

		// Send only the most urgent notification: 1-day if within 24h, otherwise 7-day
		if token.ExpiresAt.Before(oneDay) || token.ExpiresAt.Equal(oneDay) {
			if err := sendTokenExpiryNotificationIfNew(s, u, &APITokenExpiringDayNotification{
				User:  u,
				Token: token,
			}); err != nil {
				log.Errorf(logPrefix+"Error sending 1-day notification for token %d: %s", token.ID, err)
			}
			continue
		}

		if err := sendTokenExpiryNotificationIfNew(s, u, &APITokenExpiringWeekNotification{
			User:  u,
			Token: token,
		}); err != nil {
			log.Errorf(logPrefix+"Error sending 7-day notification for token %d: %s", token.ID, err)
		}
	}

	if err := s.Commit(); err != nil {
		log.Errorf(logPrefix+"Error committing session: %s", err)
	}
}

// sendTokenExpiryNotificationIfNew checks whether a notification with the same
// name and subject (token ID) has already been sent for this user. If not, it
// sends the notification (both email and DB).
func sendTokenExpiryNotificationIfNew(s *xorm.Session, u *user.User, n notifications.NotificationWithSubject) error {
	existing, err := notifications.GetNotificationsForNameAndUser(s, u.ID, n.Name(), n.SubjectID())
	if err != nil {
		return err
	}

	if len(existing) > 0 {
		return nil
	}

	return notifications.Notify(u, n, s)
}
