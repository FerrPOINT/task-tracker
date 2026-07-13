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
	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/events"
	"github.com/FerrPOINT/task-tracker/pkg/user"

	"xorm.io/xorm"
)

// CreateUserBody wraps user.APIUserPassword with admin-only fields.
type CreateUserBody struct {
	// The full name of the new user. Optional.
	Name string `json:"name" doc:"The full name of the new user. Optional."`
	// The language of the new user. Must be a valid IETF BCP 47 language code and exist in Vikunja.
	Language string `json:"language" valid:"language" doc:"IETF BCP 47 language code; must exist in Task Tracker."`
	user.APIUserPassword
	// Mark the new user as an instance admin.
	IsAdmin bool `json:"is_admin" doc:"Mark the new user as an instance admin."`
	// Activate the new user immediately without email confirmation.
	SkipEmailConfirm bool `json:"skip_email_confirm" doc:"Activate the new user immediately, skipping email confirmation."`
}

// CreateUserAsAdmin provisions a new local account on behalf of an instance admin,
// honouring the admin-only is_admin and skip_email_confirm fields and bypassing the
// public-registration toggle. It commits s and returns the persisted user reloaded
// so the status reflects what was actually stored.
func CreateUserAsAdmin(s *xorm.Session, doer *user.User, body *CreateUserBody) (*user.User, error) {
	newUser, err := RegisterUser(s, &user.User{
		Username: body.Username,
		Password: body.Password,
		Email:    body.Email,
		Name:     body.Name,
		Language: body.Language,
	})
	if err != nil {
		return nil, err
	}

	if body.IsAdmin {
		if _, err := s.ID(newUser.ID).Cols("is_admin").Update(&user.User{IsAdmin: true}); err != nil {
			return nil, err
		}
		newUser.IsAdmin = true
	}

	// Force Active when the admin asked to skip, or when no mailer exists to send the confirmation.
	if body.SkipEmailConfirm || !config.MailerEnabled.GetBool() {
		if err := user.SetUserStatus(s, newUser, user.StatusActive); err != nil {
			return nil, err
		}
		newUser.Status = user.StatusActive
	}

	// Queued alongside the user.created event RegisterUser dispatched; both
	// fire when the caller runs DispatchPending after this commit.
	events.DispatchOnCommit(s, &AdminUserCreatedEvent{User: newUser, Doer: doer})

	if err := s.Commit(); err != nil {
		return nil, err
	}

	// Reload on a fresh session so the returned status reflects what was actually
	// persisted (e.g. StatusEmailConfirmationRequired on mail-enabled instances).
	rs := db.NewSession()
	defer rs.Close()
	return user.GetUserByID(rs, newUser.ID)
}
