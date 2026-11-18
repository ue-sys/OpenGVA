// The diagrams of OpenGVA's design. A diagram exists because a view here
// asks for it; the pages the pipeline publishes are these.

package OpenGVAViews {
    import OpenGVAStructure::*;
    import OpenGVABehaviour::*;
    import OpenGVARequirements::*;

    viewpoint ContainmentVP {
        doc /* The model's packages and what is in them. */
        attribute diagram = "tree";
        expose OpenGVAStructure;
        expose OpenGVABehaviour;
        expose OpenGVARequirements;
    }

    view def CrewStationView {
        doc /* A crew station: the image, the software running from it, and the display, touch screen and bezel. */
        attribute diagram = "ibd";
        expose CrewStation::**;
    }

    view def SoftwareView {
        doc /* As it runs: logind, cage, the HMI, greetd, the programs whose windows it shows, Cockpit. */
        attribute diagram = "ibd";
        expose CrewStationSoftware::**;
    }

    view def HmiView {
        doc /* Inside opengva-hmi: hmi-core owns all state and emits a Frame; the UI only draws it. */
        attribute diagram = "ibd";
        expose OpenGvaHmi::**;
    }

    view def ImageLayersView {
        doc /* The image's layers, each a Containerfile on the one before; others' go between hmi and hardened. */
        attribute diagram = "bdd";
        expose CrewStationImage;
        expose ImageLayer;
        expose RpmPacker;
        expose BaseLayer;
        expose HmiLayer;
        expose HardenedLayer;
    }

    view def DisplayProgramsView {
        doc /* What a functional area can show. */
        attribute diagram = "bdd";
        expose DisplayProgram;
        expose VncViewer;
        expose WebViewer;
    }

    viewpoint BootVP {
        doc /* Power on to first frame, and what waits for it. */
        attribute diagram = "action";
        expose BootToFirstFrame::**;
    }

    viewpoint DisplayStateVP {
        doc /* The display is always started again. */
        attribute diagram = "state";
        expose DisplayState::**;
    }

    viewpoint AreaAccessVP {
        doc /* A role-locked area. */
        attribute diagram = "state";
        expose AreaAccess::**;
    }

    viewpoint OpenLockedAreaVP {
        doc /* Opening a locked area: the PIN, to greetd and back. */
        attribute diagram = "sequence";
        expose OpenLockedArea::**;
    }

    viewpoint ProgramButtonVP {
        doc /* A program's own button: the HMI draws its label, tells it when it is pressed, and draws what it says back. */
        attribute diagram = "sequence";
        expose ProgramButton::**;
    }

    viewpoint AlarmRaisedVP {
        doc /* An alarm from the vehicle, shown and acknowledged. */
        attribute diagram = "sequence";
        expose AlarmRaised::**;
    }

    viewpoint UseCasesVP : GeneralView {
        doc /* Who uses OpenGVA, and for what. */
        expose Crew;
        expose Maintainer;
        expose Integrator;
        expose OperateCrewStation;
        expose OpenLockedAreaCase;
        expose InspectWithCockpit;
        expose AddFunctionalArea;
        expose BuildOnImage;
    }

    viewpoint RequirementsVP {
        doc /* The requirements, what satisfies each, and how it is checked. */
        attribute diagram = "requirement";
        expose OpenGVARequirements;
    }
}
