use core::panic;
use std::io::Cursor;

use anyhow::Result;
use bit_struct::*;
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::Bytes;
use chrono::{DateTime, Utc};

use crate::{error::Error, frame::asdu::TypeID, interface::Connect};

use super::{
    asdu::{Asdu, Cause, CauseOfTransmission, CommonAddr, Identifier, InfoObjAddr, VariableStruct},
    time::{cp56time2a, decode_cp56time2a},
};

// 在控制方向过程信息的应用服务数据单元

// 单命令 信息体
#[derive(Debug, PartialEq)]
pub struct SingleCommandInfo {
    pub ioa: InfoObjAddr,
    pub sco: ObjectSCO,
    pub time: Option<DateTime<Utc>>,
}

// 双命令 信息体
#[derive(Debug, PartialEq)]
pub struct DoubleCommandInfo {
    pub ioa: InfoObjAddr,
    pub dco: ObjectDCO,
    pub time: Option<DateTime<Utc>>,
}

// 设置命令，规一化值 信息体
#[derive(Debug, PartialEq)]
pub struct SetpointCommandNormalInfo {
    pub ioa: InfoObjAddr,
    pub nva: i16,
    pub qos: ObjectQOS,
    pub time: Option<DateTime<Utc>>,
}

// 设定命令,标度化值 信息体
#[derive(Debug, PartialEq)]
pub struct SetpointCommandScaledInfo {
    pub ioa: InfoObjAddr,
    pub sva: i16,
    pub qos: ObjectQOS,
    pub time: Option<DateTime<Utc>>,
}

// 设定命令, 短浮点数 信息体
pub struct SetpointCommandFloatInfo {
    pub ioa: InfoObjAddr,
    pub r: f32,
    pub qos: ObjectQOS,
    pub time: Option<DateTime<Utc>>,
}

// 单命令 遥控信息
bit_struct! {
    pub struct ObjectSCO(u8) {
        scs: u1,    // 控制状态
        res: u1,    // 预留: 置0
        qu: u5,     // 输出方式: 0:被控确定, 1:短脉冲, 2:长脉冲, 3:持续脉冲
        se: u1,     // 选择标志: 0:执行, 1:选择
    }
}

// 双命令 遥控信息
bit_struct! {
    pub struct ObjectDCO(u8) {
        /// 控制状态
        dcs: u2,
        /// 输出方式: 0:被控确定, 1:短脉冲, 2:长脉冲, 3:持续脉冲
        qu: u5,
        /// 选择标志: 0:执行, 1:选择
        se: u1,
    }
}

// 命令限定词
bit_struct! {
    pub struct ObjectQOC(u8) {
        /// 预留：置0
        res: u2,
        /// 输出方式: 0:被控确定, 1:短脉冲, 2:长脉冲, 3:持续脉冲
        qu: u5,
        /// 选择标志: 0:执行, 1:选择
        se: u1,
    }
}

// 设定命令限定词
bit_struct! {
    pub struct ObjectQOS(u8) {
        /// 0: 默认 1-63: 预留为标准定义 64-127:特殊使用
        ql: u7,
        /// 选择标志: 0:执行, 1:选择
        se: u1,
    }
}

// SingleCmd sends a type identification [C_SC_NA_1] or [C_SC_TA_1]. 单命令, 只有单个信息对象(SQ = 0)
// [C_SC_NA_1] See companion standard 101, subclass 7.3.2.1
// [C_SC_TA_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
pub async fn single_cmd(
    c: &impl Connect,
    type_id: TypeID,
    cot: CauseOfTransmission,
    ca: CommonAddr,
    cmd: SingleCommandInfo,
) -> Result<(), Error> {
    let mut cot = cot;
    let cause = cot.cause().get();

    if !(cause == Cause::Activation || cause == Cause::Deactivation) {
        return Err(Error::ErrCmdCause(cot));
    }

    let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());

    let mut buf = vec![];
    buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
    buf.write_u8(cmd.sco.raw())?;

    match type_id {
        TypeID::C_SC_NA_1 => (),
        TypeID::C_SC_TA_1 => {
            if let Some(time) = cmd.time {
                buf.extend_from_slice(&cp56time2a(time));
            } else {
                buf.extend_from_slice(&cp56time2a(Utc::now()));
            }
        }
        _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
    }

    let asdu = Asdu {
        identifier: Identifier {
            type_id,
            variable_struct,
            cot,
            orig_addr: 0,
            common_addr: ca,
        },
        raw: Bytes::from(buf),
    };

    c.send(asdu).await
}

// DoubleCmd sends a type identification [C_DC_NA_1] or [C_DC_TA_1]. 双命令, 只有单个信息对象(SQ = 0)
// [C_DC_NA_1] See companion standard 101, subclass 7.3.2.2
// [C_DC_TA_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
pub async fn double_cmd(
    c: &impl Connect,
    type_id: TypeID,
    cot: CauseOfTransmission,
    ca: CommonAddr,
    cmd: DoubleCommandInfo,
) -> Result<(), Error> {
    let mut cot = cot;
    let cause = cot.cause().get();

    if !(cause == Cause::Activation || cause == Cause::Deactivation) {
        return Err(Error::ErrCmdCause(cot));
    }

    let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());

    let mut buf = vec![];
    buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
    buf.write_u8(cmd.dco.raw())?;

    match type_id {
        TypeID::C_DC_NA_1 => (),
        TypeID::C_DC_TA_1 => {
            if let Some(time) = cmd.time {
                buf.extend_from_slice(&cp56time2a(time));
            } else {
                buf.extend_from_slice(&cp56time2a(Utc::now()));
            }
        }
        _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
    }

    let asdu = Asdu {
        identifier: Identifier {
            type_id,
            variable_struct,
            cot,
            orig_addr: 0,
            common_addr: ca,
        },
        raw: Bytes::from(buf),
    };

    c.send(asdu).await
}

// StepCmd sends a type [C_RC_NA_1] or [C_RC_TA_1]. 步调节命令, 只有单个信息对象(SQ = 0)
// [C_RC_NA_1] See companion standard 101, subclass 7.3.2.3
// [C_RC_TA_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
// pub async fn step_cmd(
//     c: &impl Connect,
//     type_id: TypeID,
//     cot: CauseOfTransmission,
//     ca: CommonAddr,
//     cmd: StepCommandInfo,
// ) -> Result<(), Error> {
//     let mut cot = cot;
//     let cause = cot.cause().get();
//
//     if !(cause == Cause::Activation || cause == Cause::Deactivation) {
//         return Err(Error::ErrCmdCause(cot));
//     }
//
//     let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());
//
//     let mut buf = vec![];
//     buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
//     buf.write_u8(cmd.dco.raw())?;
//
//     match type_id {
//         TypeID::C_DC_NA_1 => (),
//         TypeID::C_DC_TA_1 => {
//             if let Some(time) = cmd.time {
//                 buf.extend_from_slice(&cp56time2a(time));
//             } else {
//                 buf.extend_from_slice(&cp56time2a(Utc::now()));
//             }
//         }
//         _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
//     }
//
//     let asdu = Asdu {
//         identifier: Identifier {
//             type_id,
//             variable_struct,
//             cot,
//             orig_addr: 0,
//             common_addr: ca,
//         },
//         raw: Bytes::from(buf),
//     };
//
//     c.send(asdu).await
// }

// SetpointCmdNormal sends a type [C_SE_NA_1] or [C_SE_TA_1]. 设定命令,规一化值, 只有单个信息对象(SQ = 0)
// [C_SE_NA_1] See companion standard 101, subclass 7.3.2.4
// [C_SE_TA_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
pub async fn set_point_cmd_normal(
    c: &impl Connect,
    type_id: TypeID,
    cot: CauseOfTransmission,
    ca: CommonAddr,
    cmd: SetpointCommandNormalInfo,
) -> Result<(), Error> {
    let mut cot = cot;
    let cause = cot.cause().get();

    if !(cause == Cause::Activation || cause == Cause::Deactivation) {
        return Err(Error::ErrCmdCause(cot));
    }

    let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());

    let mut buf = vec![];
    buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
    buf.write_i16::<LittleEndian>(cmd.nva)?;
    buf.write_u8(cmd.qos.raw())?;

    match type_id {
        TypeID::C_SE_NA_1 => (),
        TypeID::C_SE_TA_1 => {
            if let Some(time) = cmd.time {
                buf.extend_from_slice(&cp56time2a(time));
            } else {
                buf.extend_from_slice(&cp56time2a(Utc::now()));
            }
        }
        _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
    }

    let asdu = Asdu {
        identifier: Identifier {
            type_id,
            variable_struct,
            cot,
            orig_addr: 0,
            common_addr: ca,
        },
        raw: Bytes::from(buf),
    };

    c.send(asdu).await
}

// SetpointCmdScaled sends a type [C_SE_NB_1] or [C_SE_TB_1]. 设定命令,标度化值,只有单个信息对象(SQ = 0)
// [C_SE_NB_1] See companion standard 101, subclass 7.3.2.5
// [C_SE_TB_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
pub async fn set_point_cmd_scaled(
    c: &impl Connect,
    type_id: TypeID,
    cot: CauseOfTransmission,
    ca: CommonAddr,
    cmd: SetpointCommandScaledInfo,
) -> Result<(), Error> {
    let mut cot = cot;
    let cause = cot.cause().get();

    if !(cause == Cause::Activation || cause == Cause::Deactivation) {
        return Err(Error::ErrCmdCause(cot));
    }

    let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());

    let mut buf = vec![];
    buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
    buf.write_i16::<LittleEndian>(cmd.sva)?;
    buf.write_u8(cmd.qos.raw())?;

    match type_id {
        TypeID::C_SE_NB_1 => (),
        TypeID::C_SE_TB_1 => {
            if let Some(time) = cmd.time {
                buf.extend_from_slice(&cp56time2a(time));
            } else {
                buf.extend_from_slice(&cp56time2a(Utc::now()));
            }
        }
        _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
    }

    let asdu = Asdu {
        identifier: Identifier {
            type_id,
            variable_struct,
            cot,
            orig_addr: 0,
            common_addr: ca,
        },
        raw: Bytes::from(buf),
    };

    c.send(asdu).await
}

// SetpointCmdFloat sends a type [C_SE_NC_1] or [C_SE_TC_1].设定命令,短浮点数,只有单个信息对象(SQ = 0)
// [C_SE_NC_1] See companion standard 101, subclass 7.3.2.6
// [C_SE_TC_1] See companion standard 101,
// 传送原因(coa)用于
// 控制方向：
// <6> := 激活
// <8> := 停止激活
// 监视方向：
// <7> := 激活确认
// <9> := 停止激活确认
// <10> := 激活终止
// <44> := 未知的类型标识
// <45> := 未知的传送原因
// <46> := 未知的应用服务数据单元公共地址
// <47> := 未知的信息对象地址
pub async fn set_point_cmd_float(
    c: &impl Connect,
    type_id: TypeID,
    cot: CauseOfTransmission,
    ca: CommonAddr,
    cmd: SetpointCommandFloatInfo,
) -> Result<(), Error> {
    let mut cot = cot;
    let cause = cot.cause().get();

    if !(cause == Cause::Activation || cause == Cause::Deactivation) {
        return Err(Error::ErrCmdCause(cot));
    }

    let variable_struct = VariableStruct::new(u1::new(0).unwrap(), u7::new(1).unwrap());

    let mut buf = vec![];
    buf.write_u24::<LittleEndian>(cmd.ioa.raw().value())?;
    buf.write_f32::<LittleEndian>(cmd.r)?;
    buf.write_u8(cmd.qos.raw())?;

    match type_id {
        TypeID::C_SE_NC_1 => (),
        TypeID::C_SE_TC_1 => {
            if let Some(time) = cmd.time {
                buf.extend_from_slice(&cp56time2a(time));
            } else {
                buf.extend_from_slice(&cp56time2a(Utc::now()));
            }
        }
        _ => return Err(Error::ErrTypeIDNotMatch(type_id)),
    }

    let asdu = Asdu {
        identifier: Identifier {
            type_id,
            variable_struct,
            cot,
            orig_addr: 0,
            common_addr: ca,
        },
        raw: Bytes::from(buf),
    };

    c.send(asdu).await
}

impl Asdu {
    // [C_SC_NA_1] or [C_SC_TA_1] 获取单命令信息体
    pub fn get_single_cmd(&mut self) -> Result<SingleCommandInfo> {
        let mut rdr = Cursor::new(&self.raw);
        let ioa =
            InfoObjAddr::try_from(u24::new(rdr.read_u24::<LittleEndian>()?).unwrap()).unwrap();
        let sco = ObjectSCO::try_from(rdr.read_u8()?).unwrap();

        let mut time = None;
        match self.identifier.type_id {
            TypeID::C_SC_NA_1 => (),
            TypeID::C_SC_TA_1 => time = decode_cp56time2a(&mut rdr)?,
            _ => panic!("ErrTypeIDNotMatch"),
        }
        Ok(SingleCommandInfo { ioa, sco, time })
    }

    // [C_DC_NA_1] or [C_DC_TA_1] 获取双命令信息体
    pub fn get_double_cmd(&mut self) -> Result<DoubleCommandInfo> {
        let mut rdr = Cursor::new(&self.raw);
        let ioa =
            InfoObjAddr::try_from(u24::new(rdr.read_u24::<LittleEndian>()?).unwrap()).unwrap();
        let dco = ObjectDCO::try_from(rdr.read_u8()?).unwrap();
        let mut time = None;
        match self.identifier.type_id {
            TypeID::C_DC_NA_1 => (),
            TypeID::C_DC_TA_1 => time = decode_cp56time2a(&mut rdr)?,
            _ => panic!("ErrTypeIDNotMatch"),
        }
        Ok(DoubleCommandInfo { ioa, dco, time })
    }

    // GetSetpointNormalCmd [C_SE_NA_1] or [C_SE_TA_1] 获取设定命令,规一化值信息体
    pub fn get_setpoint_normal_cmd(&mut self) -> Result<SetpointCommandNormalInfo> {
        let mut rdr = Cursor::new(&self.raw);
        let ioa =
            InfoObjAddr::try_from(u24::new(rdr.read_u24::<LittleEndian>()?).unwrap()).unwrap();
        let nva = rdr.read_i16::<LittleEndian>()?;
        let qos = ObjectQOS::try_from(rdr.read_u8()?).unwrap();

        let mut time = None;
        match self.identifier.type_id {
            TypeID::C_SE_NA_1 => (),
            TypeID::C_SE_TA_1 => time = decode_cp56time2a(&mut rdr)?,
            _ => panic!("ErrTypeIDNotMatch"),
        }

        Ok(SetpointCommandNormalInfo {
            ioa,
            nva,
            qos,
            time,
        })
    }

    // [C_SE_NB_1] or [C_SE_TB_1] 获取设定命令,标度化值信息体
    pub fn get_setpoint_scaled_cmd(&mut self) -> Result<SetpointCommandScaledInfo> {
        let mut rdr = Cursor::new(&self.raw);
        let ioa =
            InfoObjAddr::try_from(u24::new(rdr.read_u24::<LittleEndian>()?).unwrap()).unwrap();
        let sva = rdr.read_i16::<LittleEndian>()?;
        let qos = ObjectQOS::try_from(rdr.read_u8()?).unwrap();

        let mut time = None;
        match self.identifier.type_id {
            TypeID::C_SE_NB_1 => (),
            TypeID::C_SE_TB_1 => time = decode_cp56time2a(&mut rdr)?,
            _ => panic!("ErrTypeIDNotMatch"),
        }

        Ok(SetpointCommandScaledInfo {
            ioa,
            sva,
            qos,
            time,
        })
    }

    // [C_SE_NC_1] or [C_SE_TC_1] 获取设定命令，短浮点数信息体
    pub fn get_setpoint_float_cmd(&mut self) -> Result<SetpointCommandFloatInfo> {
        let mut rdr = Cursor::new(&self.raw);
        let ioa =
            InfoObjAddr::try_from(u24::new(rdr.read_u24::<LittleEndian>()?).unwrap()).unwrap();
        let r = rdr.read_f32::<LittleEndian>()?;
        let qos = ObjectQOS::try_from(rdr.read_u8()?).unwrap();

        let mut time = None;
        match self.identifier.type_id {
            TypeID::C_SE_NC_1 => (),
            TypeID::C_SE_TC_1 => time = decode_cp56time2a(&mut rdr)?,
            _ => panic!("ErrTypeIDNotMatch"),
        }

        Ok(SetpointCommandFloatInfo { ioa, r, qos, time })
    }
}
