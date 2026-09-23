PREFIX ?= /usr/local
CARGO ?= cargo
.PHONY: build test install uninstall
build:
	$(CARGO) build --release --locked
test:
	$(CARGO) test --locked
install:
	install -Dm755 target/release/nagi "$(DESTDIR)$(PREFIX)/bin/nagi"
	install -Dm644 packaging/io.github.tcballard.Nagi.desktop "$(DESTDIR)$(PREFIX)/share/applications/io.github.tcballard.Nagi.desktop"
	./scripts/install-icons.sh "$(DESTDIR)$(PREFIX)/share/icons/hicolor"
	install -Dm644 LICENSE "$(DESTDIR)$(PREFIX)/share/licenses/nagi/LICENSE"
uninstall:
	rm -f "$(DESTDIR)$(PREFIX)/bin/nagi"
	rm -f "$(DESTDIR)$(PREFIX)/share/applications/io.github.tcballard.Nagi.desktop"
	./scripts/install-icons.sh "$(DESTDIR)$(PREFIX)/share/icons/hicolor" --uninstall
	rm -f "$(DESTDIR)$(PREFIX)/share/licenses/nagi/LICENSE"
