@echo off
git config core.hooksPath .githooks
if errorlevel 1 exit /b %errorlevel%
echo Git hooks enabled from .githooks
