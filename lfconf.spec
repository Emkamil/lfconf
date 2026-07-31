Name:           lfconf
Version:        0.1.0
Release:        1%{?dist}
Summary:        Configuration daemon and CLI tool for LFBE Desktop

License:        GPL-3.0-or-later
URL:            https://github.com/Emkamil/lfconf

Source:         {{{ git_dir_pack }}}

BuildRequires:  cargo
BuildRequires:  rust
BuildRequires:  pkgconfig(dbus-1)

%description
Lfconfd is a lightweight configuration daemon for the LFBE desktop, communicating via D-Bus. The package also includes lfconf-query –
a CLI tool for reading and writing settings.

%prep
%setup -q -n %{name}

%build
cargo build --release --workspace

%install
install -D -m 0755 target/release/lfconfd %{buildroot}%{_bindir}/lfconfd
install -D -m 0755 target/release/lfconf-query %{buildroot}%{_bindir}/lfconf-query
install -D -m 0644 res/org.lfbe.lfconf.service \
    %{buildroot}%{_datadir}/dbus-1/services/org.lfbe.lfconf.service
install -D -m 0644 res/defaults.ron \
    %{buildroot}%{_datadir}/lfconf/defaults.ron

%files
%license LICENSE
%{_bindir}/lfconfd
%{_bindir}/lfconf-query
%{_datadir}/dbus-1/services/org.lfbe.lfconf.service
%{_datadir}/lfconf/defaults.ron

%changelog
* Fri Jul 31 2026 Twoje Imię <ty@example.com> - 0.1.0-1
- Pierwsze wydanie
