# Database migration commands
SQLX := sqlx

# Add a new migration
.PHONY: migrate-add

migrate-add:
	@cmd /V:ON /C "set /p name=Enter migration name: && $(SQLX) migrate add !name!"

# .PHONY: migrate-add
# migrate-add:
# 	@echo @echo off > tmp_migrate.bat
# 	@echo set /p name="Enter migration name: " >> tmp_migrate.bat
# 	@echo $(SQLX) migrate add %%name%% >> tmp_migrate.bat
# 	@call tmp_migrate.bat
# 	@del tmp_migrate.bat

# Run all pending migrations
.PHONY: migrate-run
migrate-run:
	$(SQLX) migrate run

# Revert the last migration
.PHONY: migrate-revert
migrate-revert:
	$(SQLX) migrate revert

# Check the current migration status
.PHONY: migrate-status
migrate-status:
	$(SQLX) migrate status

# Help target
.PHONY: help
help:
	@echo "Available database migration commands:"
	@echo "  make migrate-add    - Add a new migration"
	@echo "  make migrate-run    - Run all pending migrations"
	@echo "  make migrate-revert - Revert the last migration"
	@echo "  make migrate-status - Check the current migration status"
