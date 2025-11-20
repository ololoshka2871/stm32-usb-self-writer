// Host tests for core settings
use corelogic::app_settings::*;

#[test]
fn monitoring_is_set() {
    let m = Monitoring {
        Ovarpress: false,
        Ovarheat: false,
        CPUOvarheat: false,
        OverPower: false,
    };
    assert!(!m.is_set());

    let m2 = Monitoring {
        Ovarpress: true,
        Ovarheat: false,
        CPUOvarheat: false,
        OverPower: false,
    };
    assert!(m2.is_set());
}

#[test]
fn appsettings_basic_values() {
    let s = AppSettings {
        Serial: 123,
        PMesureTime_ms: 10,
        T1MesureTime_ms: 20,
        T2MesureTime_ms: 30,
        Fref: 48000000,
        P_enabled: true,
        T1_enabled: true,
        T2_enabled: true,
        TCPUEnabled: true,
        VBatEnabled: false,
        P_Coefficients: P16Coeffs { Fp0: 0.0, Ft0: 0.0, A: [0.0; 16] },
        T1_Coefficients: T5Coeffs { F0: 0.0, T0: 0.0, C: [0.0; 5] },
        T2_Coefficients: T5Coeffs { F0: 0.0, T0: 0.0, C: [0.0; 5] },
        PWorkRange: WorkRange { minimum: 0.0, maximum: 1.0, absolute_maximum: 2.0 },
        TWorkRange: WorkRange { minimum: 0.0, maximum: 1.0, absolute_maximum: 2.0 },
        TCPUWorkRange: WorkRange { minimum: 0.0, maximum: 1.0, absolute_maximum: 2.0 },
        VbatWorkRange: WorkRange { minimum: 0.0, maximum: 1.0, absolute_maximum: 2.0 },
        PZeroCorrection: 0.0,
        TZeroCorrection: 0.0,
        calibration_date: CalibrationDate { Day: 1, Month: 1, Year: 2025 },
        writeConfig: WriteConfig { BaseInterval_ms: 1000, PWriteDevider: 1, TWriteDevider: 1 },
        startDelay: 0,
        pressureMeassureUnits: PressureMeassureUnits::Pa,
        password: [0u8; 10],
        monitoring: Monitoring { Ovarpress: false, Ovarheat: false, CPUOvarheat: false, OverPower: false },
    };

    assert_eq!(s.Serial, 123);
    assert!(s.P_enabled);
}
