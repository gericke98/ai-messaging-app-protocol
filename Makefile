# Phase 14 UAT automation harness orchestrator.
# See docs/testing.md for full usage.

.PHONY: uat uat-clean uat-debug

UAT_SERVER_LOG       := tmp/uat-server.log
UAT_SERVER_PID       := tmp/uat-server.pid
UAT_PORT             := 8080
UAT_HEALTH_URL       := http://localhost:$(UAT_PORT)/health
UAT_HEALTH_TIMEOUT_S := 30
UAT_SCHEME           := AINMMessengerUITests
UAT_DESTINATION      := 'platform=iOS Simulator,name=iPhone 16,OS=18.0'
UAT_RESULT_BUNDLE    := tmp/UAT-Results

uat: ## Run hermetic UAT suite (boot server, poll health, run xcodebuild test, teardown)
	@mkdir -p tmp
	@# Preflight: fail fast if port is in use (Pitfall 8 -- tokio bind would otherwise fail silently in background)
	@if lsof -nP -iTCP:$(UAT_PORT) -sTCP:LISTEN >/dev/null 2>&1; then \
	  echo "[uat] port $(UAT_PORT) already in use. Run 'lsof -i :$(UAT_PORT)' to identify; aborting." >&2 ; exit 1 ; \
	fi
	@# Start server in background; capture PID so the trap can kill it.
	@( cd demo && cargo run --quiet --bin ainm-demo -- --uat > ../$(UAT_SERVER_LOG) 2>&1 & echo $$! > ../$(UAT_SERVER_PID) )
	@trap 'echo "[uat] tearing down server (PID $$(cat $(UAT_SERVER_PID) 2>/dev/null))"; kill $$(cat $(UAT_SERVER_PID) 2>/dev/null) 2>/dev/null; rm -f $(UAT_SERVER_PID)' EXIT INT TERM ; \
	 echo "[uat] waiting for server readiness at $(UAT_HEALTH_URL) (timeout $(UAT_HEALTH_TIMEOUT_S)s)" ; \
	 for i in $$(seq 1 $(UAT_HEALTH_TIMEOUT_S)) ; do \
	   if curl -sf -o /dev/null "$(UAT_HEALTH_URL)" ; then \
	     echo "[uat] server ready after $$i second(s)" ; break ; \
	   fi ; \
	   if [ $$i -eq $(UAT_HEALTH_TIMEOUT_S) ] ; then \
	     echo "[uat] server not ready after $(UAT_HEALTH_TIMEOUT_S)s (tail of $(UAT_SERVER_LOG) below):" >&2 ; \
	     tail -20 $(UAT_SERVER_LOG) >&2 || true ; \
	     exit 1 ; \
	   fi ; \
	   sleep 1 ; \
	 done ; \
	 echo "[uat] running xcodebuild test -scheme $(UAT_SCHEME)..." ; \
	 ( cd ios/AINMMessenger && xcodebuild test \
	     -project AINMMessenger.xcodeproj \
	     -scheme $(UAT_SCHEME) \
	     -destination $(UAT_DESTINATION) \
	     CODE_SIGNING_ALLOWED=NO \
	     -resultBundlePath ../../$(UAT_RESULT_BUNDLE) )

uat-debug: ## Boot the UAT server and hold it until Ctrl-C (for manual inspection; no tests)
	@mkdir -p tmp
	@echo "[uat-debug] booting ainm-demo --uat on port $(UAT_PORT); Ctrl-C to stop"
	@cd demo && cargo run --bin ainm-demo -- --uat

uat-clean: ## Remove UAT artifacts (logs, result bundles, PID file)
	rm -rf tmp/
