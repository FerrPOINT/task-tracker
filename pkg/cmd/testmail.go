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

package cmd

import (
	"strings"

	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/initialize"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/mail"
	"github.com/FerrPOINT/task-tracker/pkg/notifications"
	"github.com/spf13/cobra"
)

func init() {
	rootCmd.AddCommand(testmailCmd)
}

var testmailCmd = &cobra.Command{
	Use:   "testmail [email]",
	Short: "Send a test mail using the configured smtp connection",
	Args:  cobra.ExactArgs(1),
	PreRun: func(_ *cobra.Command, _ []string) {
		initialize.LightInit()

		// Start the mail daemon
		mail.StartMailDaemon()
	},
	Run: func(_ *cobra.Command, args []string) {
		log.Info("Sending testmail...")
		message := notifications.NewMail().
			From("Task Tracker <"+config.MailerFromEmail.GetString()+">").
			To(args[0]).
			Subject("Test from Task Tracker").
			Line("This is a test mail!").
			Line("If you received this, Task Tracker is correctly set up to send emails.").
			Action("Go to your instance", config.ServicePublicURL.GetString())

		opts, err := notifications.RenderMail(message, "en")
		if err != nil {
			log.Errorf("Error rendering test mail: %s", err.Error())
			return
		}
		if err := mail.SendTestMail(opts); err != nil &&
			!strings.HasPrefix(err.Error(), "failed to close connction: not connected to SMTP server") {
			log.Errorf("Error sending test mail: %s", err.Error())
			return
		}
		log.Info("Testmail successfully sent.")
	},
}
