#!/usr/bin/env python3
"""Verify installed hicolor lookup under the GUI test display."""
import gi, os
from pathlib import Path
gi.require_version('Gtk','4.0')
from gi.repository import Gtk, Gdk
Gtk.init()
theme=Gtk.IconTheme.get_for_display(Gdk.Display.get_default())
base=Path(os.environ['XDG_DATA_HOME'])/'icons/hicolor'
for size in (16,22,24,32,48,64,128,256,512):
    icon=theme.lookup_icon('nagi',[],size,1,Gtk.TextDirection.NONE,Gtk.IconLookupFlags.FORCE_REGULAR)
    assert Path(icon.get_file().get_path())==base/f'{size}x{size}/apps/nagi.png'
assert theme.has_icon('nagi-symbolic')
print('Icon=nagi resolves every installed size; symbolic icon is discoverable.')
