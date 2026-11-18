// OpenGVA's structure: what a crew station is made of, from the image's
// layers down to the HMI's crates. What is put on the image, and the bench it
// is tried on, are OpenGVA-Examples' model, which imports this one.
//
// One package per file; the others import this one. Every diagram of it is
// a view in views.sysml.

package OpenGVAStructure {

    // --- Ports: what passes between the parts -------------------------------

    port def DisplayPort {
        doc /* A Wayland connection: windows given to the compositor to show. */
    }
    port def ScreenOut {
        doc /* The display output (DRM/KMS) the compositor owns. */
    }
    port def TouchPort {
        doc /* Touches and pointer, from libinput through the compositor. */
    }
    port def BezelPort {
        doc /* The bezel's buttons, read as key events from /dev/input. */
    }
    port def FramePort {
        doc /* hmi-core's Frame: the whole state the UI draws, and nothing else. */
    }
    port def UiEventPort {
        doc /* What the UI is touched with, back to hmi-core. */
    }
    port def FeedPort {
        doc /* JSON lines about the vehicle - alarms, threats, sensors - and the crew's answers back: /run/opengva/feed, or the virtio port org.opengva.feed. */
    }
    port def AppKeysPort {
        doc /* A program's socket (gva-fa.d: socket, key): {"pressed"|"released": id} to it; {"keys": [...]} from it, the labels of F1 to F12 while its area is on screen. */
    }
    port def RolePort {
        doc /* greetd's socket: a PIN in, a role proved or not out. */
    }
    port def SessionPort {
        doc /* A logind session: the display and input devices lent to the HMI's user. */
    }
    port def NetworkPort {
        doc /* An Ethernet interface. */
    }
    port def HttpsPort {
        doc /* Cockpit's web console, port 9090. */
    }
    port def VncPort {
        doc /* RFB: another computer's screen. */
    }
    port def SerialPort {
        doc /* The development console and boot-timing markers (not on hardened images). */
    }

    // --- The HMI: one Tauri program, a Rust core and a SolidJS UI ----------

    part def HmiCore {
        doc /* hmi-core: owns all state - areas, alarms, threats, tiles, forms - and emits a Frame. */
        attribute areasConfig = "/etc/opengva/gva-fa.d/*.conf";
        port frame : FramePort;
        port uiEvents : ~UiEventPort;
        port bezel : ~BezelPort;
        port feed : ~FeedPort;
        port apps : ~AppKeysPort;
        port entry : ~UiEventPort;
        port access : ~RolePort;
    }

    part def Ui {
        doc /* The SolidJS UI in WebKitGTK: draws the Frame, sends touches back. */
        port frame : ~FramePort;
        port events : UiEventPort;
    }

    part def BezelInput {
        doc /* bezel-input: the 28 bezel buttons, from the input group's devices. */
        port keys : BezelPort;
    }

    part def HmiFeed {
        doc /* hmi-feed: finds a line (a socket or a character device), and finds it again when it goes. One for the vehicle's feed, one for each program's socket. */
        port line : FeedPort;
        port apps : AppKeysPort;
    }

    part def HmiEntry {
        doc /* hmi-entry: forms and their fields, typed on the on-screen keyboard. */
        port fields : UiEventPort;
    }

    part def UsKeyboard {
        doc /* us-keyboard: the on-screen keyboard's keys, as a Wayland virtual keyboard. */
    }

    part def HmiAccess {
        doc /* hmi-access: a role-locked area asks for a PIN, which greetd checks. */
        port role : RolePort;
    }

    part def OpenGvaHmi {
        doc /* opengva-hmi: the Tauri shell. One window, on top, clear where other programs show through. */
        port display : DisplayPort;
        port touch : ~TouchPort;
        port bezel : ~BezelPort;
        port feed : ~FeedPort;
        port apps : ~AppKeysPort;
        port role : ~RolePort;

        part core : HmiCore;
        part ui : Ui;
        part bezelInput : BezelInput;
        part hmiFeed : HmiFeed;
        part entry : HmiEntry;
        part keyboard : UsKeyboard;
        part access : HmiAccess;

        connect core.frame to ui.frame;
        connect ui.events to core.uiEvents;
        connect bezelInput.keys to core.bezel;
        connect hmiFeed.line to core.feed;
        connect hmiFeed.apps to core.apps;
        connect entry.fields to core.entry;
        connect access.role to core.access;
    }

    // --- What the HMI runs on --------------------------------------------

    part def Cage {
        doc /* cage 0.2.1 with OpenGVA's patches: the HMI in front, other windows under it, tiled in rectangles, keyboard lent, a splash until the first frame. */
        port hmiClient : ~DisplayPort;
        port programClients : ~DisplayPort;
        port screen : ScreenOut;
        port touch : TouchPort;
        port session : ~SessionPort;
    }

    part def Logind {
        doc /* systemd-logind: the session on tty1 that is lent the display and inputs. */
        port session : SessionPort;
    }

    part def Greetd {
        doc /* greetd, as root: the verifier. Started after the first frame. */
        port verifier : ~RolePort;
    }

    part def DisplayProgram {
        doc /* A program whose window the HMI shows in a functional area: a unit of its own, bound to the display, started by on-display. */
        attribute appId;
        port display : DisplayPort;
        port keys : AppKeysPort [0..1];
    }

    part def VncViewer :> DisplayProgram {
        doc /* app/viewers/vnc: another computer's screen (vnc.d). */
        port remote : ~VncPort;
    }

    part def WebViewer :> DisplayProgram {
        doc /* app/viewers/web: a web page (web.d). */
        port network : ~NetworkPort;
    }

    part def Cockpit {
        doc /* The web console on every image, OpenGVA-branded. The maintainer account sees and changes nothing. */
        port web : HttpsPort;
    }

    // The software of a crew station, as it runs.
    part def CrewStationSoftware {
        port screen : ScreenOut;
        port touch : ~TouchPort;
        port bezel : ~BezelPort;
        port feed : ~FeedPort;
        port web : HttpsPort;

        part logind : Logind;
        part cage : Cage;
        part hmi : OpenGvaHmi;
        part greetd : Greetd;
        part programs : DisplayProgram [0..*];
        part cockpit : Cockpit;

        connect logind.session to cage.session;
        connect hmi.display to cage.hmiClient;
        connect programs.display to cage.programClients;
        connect programs.keys to hmi.apps;
        connect hmi.role to greetd.verifier;
        connect cage.screen to screen;
        connect touch to cage.touch;
        connect bezel to hmi.bezel;
        connect feed to hmi.feed;
        connect cockpit.web to web;
    }

    // --- The image: a bootable container, in layers ------------------------

    part def ImageLayer {
        doc /* A Containerfile on the one before: what it adds, it installs as RPMs. */
        attribute containerfile;
    }

    part def RpmPacker {
        doc /* opengva-rpm (os/hmi, by app/opengva.spec), which makes an RPM of a tree laid out as / is. Every compiled part of OpenGVA and every layer on it is packed by it in a stage of its own build. It is on the image; rpmbuild, which it packs with, is not. */
    }

    part def BaseLayer :> ImageLayer {
        doc /* os/base: Rocky Linux 10 (rocky-bootc-minimal), its kernel, systemd, the boot, Cockpit and the maintainer account. */
    }

    part def HmiLayer :> ImageLayer {
        doc /* os/hmi: the crew station's - cage, greetd, the HMI and its viewers. */
    }

    part def HardenedLayer :> ImageLayer {
        doc /* os/hardened, last, on top of whatever has been added: the DISA STIG (stig_gui), no root login, no serial console, nothing setuid, USBGuard by device class, fapolicyd running only what an RPM installed, unchanged (SHA-256), shell scripts included. */
    }

    part def CrewStationImage {
        doc /* make container: localhost/opengva:hmi, an OCI image to build on. Whoever uses it adds layers, hardens it or not, and makes a disk of it (osbuild's image-builder) or moves a crew station to it (bootc switch). */
        part base : BaseLayer;
        part hmi : HmiLayer;
        part added : ImageLayer [0..*];
        part hardened : HardenedLayer [0..1];
    }

    // --- The machine ------------------------------------------------------

    part def Display {
        port input : ~ScreenOut;
    }
    part def TouchScreen {
        port touches : TouchPort;
    }
    part def Bezel {
        doc /* 28 buttons round the display. */
        port buttons : BezelPort;
    }
    part def VehicleInterface {
        doc /* Whatever knows the vehicle: it holds the other end of the feed. */
        port feed : FeedPort;
    }

    part def CrewStation {
        doc /* A PC (or a virtual machine) that boots the image straight into the HMI. */
        port network : NetworkPort;
        port serial : SerialPort;

        part image : CrewStationImage;
        part software : CrewStationSoftware;
        part display : Display;
        part touchScreen : TouchScreen;
        part bezel : Bezel;

        connect software.screen to display.input;
        connect touchScreen.touches to software.touch;
        connect bezel.buttons to software.bezel;
    }

    part crewStation : CrewStation;
}
