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

package initialize

import (
	"time"

	"github.com/FerrPOINT/task-tracker/pkg/audit"
	"github.com/FerrPOINT/task-tracker/pkg/config"
	"github.com/FerrPOINT/task-tracker/pkg/cron"
	"github.com/FerrPOINT/task-tracker/pkg/db"
	"github.com/FerrPOINT/task-tracker/pkg/events"
	"github.com/FerrPOINT/task-tracker/pkg/files"
	"github.com/FerrPOINT/task-tracker/pkg/i18n"
	"github.com/FerrPOINT/task-tracker/pkg/license"
	"github.com/FerrPOINT/task-tracker/pkg/log"
	"github.com/FerrPOINT/task-tracker/pkg/mail"
	"github.com/FerrPOINT/task-tracker/pkg/migration"
	"github.com/FerrPOINT/task-tracker/pkg/models"
	"github.com/FerrPOINT/task-tracker/pkg/modules/auth/ldap"
	"github.com/FerrPOINT/task-tracker/pkg/modules/auth/openid"
	"github.com/FerrPOINT/task-tracker/pkg/modules/keyvalue"
	migrationHandler "github.com/FerrPOINT/task-tracker/pkg/modules/migration/handler"
	"github.com/FerrPOINT/task-tracker/pkg/plugins"
	"github.com/FerrPOINT/task-tracker/pkg/red"
	"github.com/FerrPOINT/task-tracker/pkg/user"
	ws "github.com/FerrPOINT/task-tracker/pkg/websocket"
)

// LightInit will only init config, redis, logger but no db connection.
func LightInit() {
	// Set logger
	log.InitLogger()

	// Init the config
	config.InitConfig()

	// Check if the configured time zone is valid
	if _, err := time.LoadLocation(config.ServiceTimeZone.GetString()); err != nil {
		log.Criticalf("Error parsing default time zone: %s", err)
	}

	// Init redis
	red.InitRedis()

	// Init keyvalue store
	keyvalue.InitStorage()
}

// InitEngines intializes all db connections
func InitEngines() {
	err := models.SetEngine()
	if err != nil {
		log.Fatal(err.Error())
	}
	err = files.SetEngine()
	if err != nil {
		log.Fatal(err.Error())
	}

	err = db.CreateParadeDBIndexes()
	if err != nil {
		log.Fatal(err.Error())
	}
}

// FullInitWithoutAsync does a full init without any async handlers (cron or events)
func FullInitWithoutAsync() {
	LightInit()

	// Initialize the files handler
	err := files.InitFileHandler()
	if err != nil {
		log.Fatalf("Could not init file handler: %s", err)
	}

	// Run the migrations
	migration.Migrate(nil)

	// Set Engine
	InitEngines()

	// Initialize license validation — funds ongoing development of Vikunja.
	// See the package comment in pkg/license/license.go before removing.
	license.Init()

	if config.AuditEnabled.GetBool() {
		if err := audit.Init(); err != nil {
			log.Fatalf("Could not initialize audit logging: %s", err)
		}
	}

	// Start the mail daemon
	mail.StartMailDaemon()

	// Connect to ldap if enabled
	ldap.InitializeLDAPConnection()

	// Check all OpenID Connect providers at startup
	_, err = openid.GetAllProviders()
	if err != nil {
		if openid.IsErrDuplicateOIDCIssuer(err) {
			log.Fatalf("OpenID Connect configuration error: %s", err)
		}
		log.Errorf("Error initializing OpenID Connect providers: %s", err)
	}

	// Load translations
	i18n.Init()

	// Initialize plugins
	plugins.Initialize()
}

// FullInit initializes all kinds of things in the right order
func FullInit() {

	FullInitWithoutAsync()

	// Start the cron
	cron.Init()
	models.RegisterReminderCron()
	models.RegisterOverdueReminderCron()
	models.RegisterUserDeletionCron()
	models.RegisterTaskCleanupCron()
	models.RegisterOldExportCleanupCron()
	models.RegisterAddTaskToFilterViewCron()
	user.RegisterTokenCleanupCron()
	models.RegisterSessionCleanupCron()
	user.RegisterDeletionNotificationCron()
	openid.CleanupSavedOpenIDProviders()
	openid.RegisterEmptyOpenIDTeamCleanupCron()
	openid.RegisterProviderAvailabilityCron()
	models.RegisterAPITokenExpiryCheckCron()

	// Initialize WebSocket hub
	ws.InitHub()

	// Start processing events
	go func() {
		models.RegisterListeners()
		migrationHandler.RegisterListeners()
		ws.RegisterListeners()
		err := events.InitEvents()
		if err != nil {
			log.Fatal(err.Error())
		}

		err = events.Dispatch(&BootedEvent{
			BootedAt: time.Now(),
		})
		if err != nil {
			log.Fatal(err)
		}
	}()
}
