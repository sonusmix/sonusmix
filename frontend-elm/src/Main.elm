port module Main exposing (main)

import Browser
import Element exposing (..)
import Element.Background as Background
import Element.Input as Input

port callIncrement : () -> Cmd msg
port returnIncrement : (Int -> msg) -> Sub msg

port callDecrement : () -> Cmd msg
port returnDecrement : (Int -> msg) -> Sub msg

type alias Model = 
    { elmCounter : Int
    , rustCounter : Int
    }

init : () -> ( Model, Cmd Msg )
init _ =
    ( Model 0 0, Cmd.none )

green : Color
green =
    Element.rgb255 0 255 0

red : Color
red =
    Element.rgb255 255 0 0

view : Model -> Browser.Document Msg
view model =
    { title = "Sonusmix"
    , body =
        [ Element.layout [] <|
            column []
                [ row []
                    [ text ("Elm Counter: " ++ String.fromInt model.elmCounter)
                    , Input.button [ Background.color green ]
                        { onPress = Just ElmCounterIncrement
                        , label = text "Increment"
                        }
                    , Input.button [ Background.color red]
                        { onPress = Just ElmCounterDecrement
                        , label = text "Decrement"
                        }
                    ]
                ,  row []
                    [ text ("Rust Counter: " ++ String.fromInt model.rustCounter)
                    , Input.button [ Background.color green ]
                        { onPress = Just RustCounterIncrement
                        , label = text "Increment"
                        }
                    , Input.button [ Background.color red]
                        { onPress = Just RustCounterDecrement
                        , label = text "Decrement"
                        }
                    ]
                ]
        ]
    }

type Msg
    = ElmCounterIncrement
    | ElmCounterDecrement
    | RustCounterIncrement
    | RustCounterDecrement
    | RustCounterReturn Int

update : Msg -> Model -> ( Model, Cmd Msg )
update msg model =
    case msg of
        ElmCounterIncrement ->
            ( { model | elmCounter = model.elmCounter + 1 }, Cmd.none )
        
        ElmCounterDecrement ->
            ( { model | elmCounter = model.elmCounter - 1 }, Cmd.none )
        
        RustCounterIncrement ->
            ( model, callIncrement () )
        
        RustCounterDecrement ->
            ( model, callDecrement () )
        
        RustCounterReturn x ->
            ( { model | rustCounter = x }, Cmd.none )

subscriptions : Model -> Sub Msg
subscriptions model =
    Sub.batch
        [ returnIncrement RustCounterReturn
        , returnDecrement RustCounterReturn
        ]

main: Program () Model Msg
main =
    Browser.document
        { init = init
        , view = view
        , update = update
        , subscriptions = subscriptions
        }
