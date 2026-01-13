export enum IframeMessageType {
    OPEN_APP = 'OPEN_APP',
    APP_LINK_CLICKED = 'APP_LINK_CLICKED',
    HW_LINK_CLICKED = 'HW_LINK_CLICKED'
}

export type IframeMessage =
    | { type: IframeMessageType.OPEN_APP, id: string }
    | { type: IframeMessageType.APP_LINK_CLICKED, url: string }
    | { type: IframeMessageType.HW_LINK_CLICKED, url: string }

export function isIframeMessage(message: any): message is IframeMessage {
    return message.type in IframeMessageType;
}
