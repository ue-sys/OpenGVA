// How a crew station comes up, how it keeps a picture on the screen, and how
// a role-locked area is opened.

package OpenGVABehaviour {
    import OpenGVAStructure::*;

    // From power on to the HMI's first frame, and what waits for it.
    action def BootToFirstFrame {
        doc /* Nothing that is not needed for the first frame starts before it. */
        action firmware;
        action grub;
        action kernel;
        action systemdInit;
        action startDisplay {
            doc /* opengva-hmi.service: a logind session on tty1, as hmi. */
        }
        action startCage;
        action startHmi;
        action firstFrame {
            doc /* The HMI has painted; cage takes the splash away. */
        }
        fork afterFrame;
        action startGreetd {
            doc /* The verifier, so that a PIN can be checked. */
        }
        action startPrograms {
            doc /* Each on-display unit: windows for the functional areas. */
        }
        join ready;
        action running;

        succession first firmware then grub;
        succession first grub then kernel;
        succession first kernel then systemdInit;
        succession first systemdInit then startDisplay;
        succession first startDisplay then startCage;
        succession first startCage then startHmi;
        succession first startHmi then firstFrame;
        succession first firstFrame then afterFrame;
        succession first afterFrame then startGreetd;
        succession first afterFrame then startPrograms;
        succession first startGreetd then ready;
        succession first startPrograms then ready;
        succession first ready then running;
    }

    // A display with nothing on it is the worst failure for an HMI.
    state def DisplayState {
        state starting;
        state showing;
        state restarting;

        transition first starting accept firstFrame then showing;
        transition first showing accept displayEnded then restarting;
        transition first restarting accept restarted then starting;
    }

    // A role-locked functional area (require <role>).
    state def AreaAccess {
        state locked;
        state askingPin;
        state open;

        transition first locked accept areaSelected then askingPin;
        transition first askingPin accept pinAccepted then open;
        transition first askingPin accept pinRefused then locked;
        transition first open accept areaLeft then locked;
    }

    item def PinEntry;
    item def RoleProved;
    item def FeedLine;
    item def Frame;

    // Opening a locked area: who asks whom.
    interaction def OpenLockedArea {
        part bezel : Bezel;
        part core : HmiCore;
        part access : HmiAccess;
        part verifier : Greetd;
        part ui : Ui;

        message areaSelected from bezel to core;
        message askPin from core to ui;
        message of PinEntry from ui to access;
        message of PinEntry from access to verifier;
        message of RoleProved from verifier to access;
        message of Frame from core to ui;
    }

    item def KeyPress;
    item def ProgramKeys;

    // A program's own button (gva-fa.d: socket, key): the HMI draws the
    // label and owns the button; the program hears of it, and relabels it.
    interaction def ProgramButton {
        part bezel : Bezel;
        part core : HmiCore;
        part feed : HmiFeed;
        part program : DisplayProgram;
        part ui : Ui;

        message of KeyPress from bezel to core;
        message of KeyPress from core to feed;
        message of KeyPress from feed to program;
        message of ProgramKeys from program to feed;
        message of ProgramKeys from feed to core;
        message of Frame from core to ui;
    }

    // The vehicle tells the HMI something.
    interaction def AlarmRaised {
        part vehicle : VehicleInterface;
        part feed : HmiFeed;
        part core : HmiCore;
        part ui : Ui;

        message of FeedLine from vehicle to feed;
        message of FeedLine from feed to core;
        message of Frame from core to ui;
        message acknowledged from ui to core;
        message of FeedLine from core to vehicle;
    }

    allocate BootToFirstFrame to CrewStation;
    allocate DisplayState to CrewStationSoftware;
    allocate AreaAccess to HmiCore;
}
