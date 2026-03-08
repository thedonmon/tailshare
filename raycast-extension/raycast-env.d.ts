/// <reference types="@raycast/api">

/* 🚧 🚧 🚧
 * This file is auto-generated from the extension's manifest.
 * Do not modify manually. Instead, update the `package.json` file.
 * 🚧 🚧 🚧 */

/* eslint-disable @typescript-eslint/ban-types */

type ExtensionPreferences = {}

/** Preferences accessible in all the extension's commands */
declare type Preferences = ExtensionPreferences

declare namespace Preferences {
  /** Preferences accessible in the `send-clipboard` command */
  export type SendClipboard = ExtensionPreferences & {}
  /** Preferences accessible in the `get-clipboard` command */
  export type GetClipboard = ExtensionPreferences & {}
  /** Preferences accessible in the `list-devices` command */
  export type ListDevices = ExtensionPreferences & {}
  /** Preferences accessible in the `send-to-device` command */
  export type SendToDevice = ExtensionPreferences & {}
}

declare namespace Arguments {
  /** Arguments passed to the `send-clipboard` command */
  export type SendClipboard = {}
  /** Arguments passed to the `get-clipboard` command */
  export type GetClipboard = {}
  /** Arguments passed to the `list-devices` command */
  export type ListDevices = {}
  /** Arguments passed to the `send-to-device` command */
  export type SendToDevice = {}
}

