import cv2
import mediapipe as mp
import socket
import json
import math

mp_hands = mp.solutions.hands
hands = mp_hands.Hands(
    max_num_hands=2,
    min_detection_confidence=0.6,
    min_tracking_confidence=0.8
)
mp_drawing = mp.solutions.drawing_utils

sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
server_address = ('127.0.0.1', 5005)

cap = cv2.VideoCapture(0)

pinch_state = {'Right': False, 'Left': False}

while cap.isOpened():
    success, image = cap.read()
    if not success:
        continue

    image = cv2.flip(image, 1)
    image_rgb = cv2.cvtColor(image, cv2.COLOR_BGR2RGB)
    results = hands.process(image_rgb)

    hand_data_list = []
    snap_detected = False

    if results.multi_hand_landmarks:
        for i, hand_landmarks in enumerate(results.multi_hand_landmarks):
            
            # 左右の判定
            handedness = results.multi_handedness[i].classification[0].label
            
            landmark_list = []
            for id, lm in enumerate(hand_landmarks.landmark):
                landmark_list.append({
                    'id': id,
                    'x': lm.x,
                    'y': lm.y,
                    'z': lm.z
                })
            
            # 1つの手のデータを辞書にする
            hand_data_list.append({
                'label': handedness,
                'landmarks': landmark_list
            })
            
            # 指パッチン判定
            th = hand_landmarks.landmark[4]
            mi = hand_landmarks.landmark[12]
            
            dist_sq = (th.x - mi.x)**2 + (th.y - mi.y)**2 + (th.z - mi.z)**2
            
            threshold_sq = 0.002 

            if dist_sq < threshold_sq:
                # くっついている
                if not pinch_state.get(handedness, False):
                    # 離れた状態からくっついた瞬間 -> スナップ検知
                    snap_detected = True
                    pinch_state[handedness] = True
            else:
                # 離れている
                pinch_state[handedness] = False

            mp_drawing.draw_landmarks(
                image,
                hand_landmarks,
                mp_hands.HAND_CONNECTIONS
            )

    if hand_data_list:
        data = json.dumps({
            'hands': hand_data_list,
            'snap': snap_detected # スナップ情報を追加
        })
        sock.sendto(data.encode('utf-8'), server_address)

    cv2.imshow('MasterHand Vision', image)
    if cv2.waitKey(5) & 0xFF == 27:
        break

cap.release()
cv2.destroyAllWindows()
