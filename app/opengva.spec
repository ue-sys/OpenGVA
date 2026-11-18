# The spec of every OpenGVA RPM: a tree laid out as / is, packed as it is.
# opengva-rpm (os/rpm) says which - the pkg_ macros - and rpmbuild works out
# what its programs need.
Name:      %{pkg_name}
Version:   %{pkg_version}
Release:   %{pkg_release}
Summary:   %{pkg_summary}
License:   %{pkg_license}
%global debug_package %{nil}
%global _build_id_links none
%description
%{pkg_summary}.
%install
cp -a "%{pkg_root}/." %{buildroot}/
%files -f %{pkg_files}
