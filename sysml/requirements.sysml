// What OpenGVA is for, who uses it, and what it has to do - each traced to
// what satisfies it and how it is checked.

package OpenGVARequirements {
    import OpenGVAStructure::*;
    import OpenGVABehaviour::*;

    // --- Who ------------------------------------------------------------

    actor Crew;
    actor Maintainer;
    actor Integrator;

    use case def OperateCrewStation {
        doc /* Read the information bar, alarms and threats; work the functional areas from the bezel or by touch. */
        subject station : CrewStation;
    }

    use case def OpenLockedAreaCase {
        doc /* Enter a PIN to open COM or DEF. */
        subject station : CrewStation;
        include OperateCrewStation;
    }

    use case def InspectWithCockpit {
        doc /* Look at a crew station's state and logs over the network, changing nothing. */
        subject station : CrewStation;
    }

    use case def AddFunctionalArea {
        doc /* Give an image another program, form, camera, VNC or web view. */
        subject image : CrewStationImage;
    }

    use case def BuildOnImage {
        doc /* make container, then layers of one's own on the image: OpenGVA-Examples is one such. */
        subject image : CrewStationImage;
    }

    // --- What -----------------------------------------------------------

    verification def MakeTest {
        doc /* make test: the HMI's Rust tests on the host. */
    }
    verification def StigScan {
        doc /* oscap xccdf eval --profile stig_gui on an image with os/hardened on top. */
    }

    requirement def <OGVA-1> GvaLayout {
        doc /* The HMI shall give the standard GVA layout - information bar, functional areas, alarms and threats - that every crew station shares. */
        subject hmi : OpenGvaHmi;

        requirement <OGVA-1.1> BezelControl {
            doc /* Every function shall be reachable from the 28 bezel buttons as well as by touch. */
        }
        requirement <OGVA-1.2> AlarmsAndThreats {
            doc /* Alarms (warning, caution, advisory) and threats from the vehicle shall be shown and acknowledged. */
        }
    }

    requirement def <OGVA-2> BootStraightIn {
        doc /* A crew station shall boot straight into the HMI with no login and nothing else first. */
        subject station : CrewStation;

        requirement <OGVA-2.1> FirstFrameFirst {
            doc /* Nothing not needed for the first frame shall start before it. */
        }
    }

    requirement def <OGVA-3> DisplayNeverEmpty {
        doc /* Whenever the display ends, however it ends, it shall be started again. */
    }

    requirement def <OGVA-4> Unprivileged {
        doc /* The HMI and the programs it shows shall run as an unprivileged user. */
    }

    requirement def <OGVA-5> PluggableAreas {
        doc /* A functional area shall be added by a layer on the image - configuration and a unit file - with no change to the HMI's code. */
    }

    requirement def <OGVA-6> RoleLockedAreas {
        doc /* An area may require a role, proved by PIN; wrong PINs shall be counted across restarts. */
    }

    requirement def <OGVA-7> Hardened {
        doc /* With os/hardened on top the image shall meet the DISA STIG for RHEL 10 (stig_gui), have no root login and no serial console. */
    }

    requirement def <OGVA-8> ReadOnlyMaintenance {
        doc /* A crew station shall be inspectable over the network by an account that can change nothing. */
    }

    requirement def <OGVA-9> NoSelfUpdate {
        doc /* Nothing shall update by itself; an image moves only by bootc switch or upgrade. */
    }

    requirement def <OGVA-10> ProgramButtons {
        doc /* A program given an area to itself shall be able to have the buttons down the sides, F1 to F12: the HMI draws their labels, as the program says them, and tells it when one is pressed and let go. */
    }

    requirement def <OGVA-11> ABaseToBuildOn {
        doc /* The image shall be a base for others' images: what they add goes in layers on it, with nothing of theirs in it. */
    }

    requirement def <OGVA-12> OnlyPackagedCode {
        doc /* On a hardened image nothing shall run that an RPM did not install, as the RPM has it: every part of OpenGVA and of the layers on it is an RPM, and the image is not built if something runnable is not. */
    }

    derive FirstFrameFirst from BootStraightIn;
    derive ProgramButtons from PluggableAreas;
    derive OnlyPackagedCode from Hardened;

    satisfy GvaLayout by OpenGvaHmi;
    satisfy BezelControl by BezelInput;
    satisfy AlarmsAndThreats by HmiCore;
    satisfy BootStraightIn by CrewStationSoftware;
    satisfy FirstFrameFirst by Greetd;
    satisfy DisplayNeverEmpty by CrewStationSoftware;
    satisfy Unprivileged by Logind;
    satisfy PluggableAreas by DisplayProgram;
    satisfy RoleLockedAreas by HmiAccess;
    satisfy Hardened by HardenedLayer;
    satisfy ReadOnlyMaintenance by Cockpit;
    satisfy NoSelfUpdate by BaseLayer;
    satisfy ProgramButtons by HmiCore;
    satisfy ABaseToBuildOn by CrewStationImage;
    satisfy OnlyPackagedCode by HardenedLayer;
    satisfy OnlyPackagedCode by RpmPacker;

    verify GvaLayout by MakeTest;
    verify AlarmsAndThreats by MakeTest;
    verify ProgramButtons by MakeTest;
    verify Hardened by StigScan;

    refine DisplayNeverEmpty by DisplayState;
}
